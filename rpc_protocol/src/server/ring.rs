// SPDX-License-Identifier: BSD-3-Clause
// Copyright 2025. Triad National Security, LLC.

use std::fmt;
use std::io;
use std::net::TcpListener;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicU16, Ordering};

use io_uring::{cqueue, opcode, types, IoUring};
use log::*;

use crate::*;

use super::{encode_succesful_reply, validate_program_and_version, RpcResult};

const GROUP_ID: u16 = 42;

/// The io_uring implementation has a custom procedure type that returns a RingResult rather than
/// the RpcResult.
pub type RingProcedure<T> = fn(&Call, &mut T) -> RingResult;
pub type RingProcedureList<T> = Vec<Option<RingProcedure<T>>>;

pub enum RingResult {
    /// A procedure implementation can either complete synchronously, in which case it returns the
    /// immediate result as an RpcResult...
    Done(RpcResult),

    /// ...or it may need to do I/O, which will use this thread's io_uring instance. The RpcServer
    /// will submit the Entry on behalf of the procedure implemenation, and call a user-supplied
    /// callback (TODO: implement this...) when the completion comes in.
    MoreIo(cqueue::Entry),
}

/// A mapping between RPC procedures (identified by program, version, and procedure numbers), and
/// the Rust code that implements them.
pub struct ProcedureMap<T> {
    /// The program number of this RPC service.
    program: u32,

    /// The min version number of this RPC service.
    version_min: u32,

    /// The max version number of this RPC service.
    version_max: u32,

    /// The mapping of procedure numbers to functions that implement the procedures.
    /// The 0th element of this array is ignored because it is always mapped to the NULL procedure.
    /// This structure assumes that al the versions between version_min and version_max share the
    /// same procedures. If that assumption should turn false in the future, this structure will
    /// have to be modified.
    procedures: RingProcedureList<T>,
}

impl<T> ProcedureMap<T> {
    pub fn new(
        program: u32,
        version_min: u32,
        version_max: u32,
        procedures: RingProcedureList<T>,
    ) -> Self {
        Self {
            program,
            version_min,
            version_max,
            procedures,
        }
    }
}

pub struct RpcServer<T> {
    ring: IoUring,
    listener: TcpListener,
    buffer_map: BufferMap,
    procedure_map: ProcedureMap<T>,

    /// The RPC service implementation uses this field to store state that must be maintained
    /// across RPC calls.
    user_state: T,
}

impl<T> RpcServer<T> {
    pub fn new(address: &str, procedure_map: ProcedureMap<T>, user_state: T) -> io::Result<Self> {
        let mut ring = IoUring::new(1024)?;
        let buffer_map = BufferMap::new(&mut ring);

        let mut ring = Self {
            ring,
            listener: TcpListener::bind(address)?,
            buffer_map,
            procedure_map,
            user_state,
        };

        ring.submit_multishot_accept();

        Ok(ring)
    }

    pub fn main_loop(&mut self) -> io::Result<()> {
        loop {
            self.try_submit_and_wait();

            let cqe = self
                .ring
                .completion()
                .next()
                .expect("failed to get completion");

            // SAFETY: user data was derived from a Box<Operation>::into_raw().
            let op = unsafe { Operation::from_u64(cqe.user_data()) };

            check_completion_error(&cqe, &op);

            trace!("{op}: {cqe:?}");

            match *op {
                Operation::Accept(ref a) => {
                    let listen_fd = a.fd;
                    op.handle_accept(&mut self.ring, cqe, listen_fd);
                }
                Operation::Recv(ref r) => {
                    let conn_fd = r.fd;
                    op.handle_receive(self, cqe, conn_fd);
                }
                Operation::Send(s) => {
                    eprintln!("send completion (not yet handling): {s:?}, {cqe:?}");
                }
            }
        }
    }

    fn submit_multishot_accept(&mut self) {
        let listen_fd = self.listener.as_raw_fd();
        let user_data = Box::new(Operation::Accept(Accept::new(listen_fd)));
        let listen_fd = types::Fd(self.listener.as_raw_fd());

        submit_accept(&mut self.ring, listen_fd, user_data.to_u64());
    }

    fn try_submit_and_wait(&mut self) {
        let Err(e) = self.ring.submit_and_wait(1) else {
            return;
        };

        match nix::errno::Errno::from_raw(e.raw_os_error().unwrap()) {
            // EAGAIN means try again later, so just return now:
            nix::Error::EAGAIN => {}
            other => {
                panic!("Unexpected error result from io_uring_enter() (submit_and_wait()): {other}")
            }
        };
    }

    /// Given `amount` bytes received in a buffer identified by `buffer_id`, try to interpret those
    /// bytes as an RPC message.
    ///
    /// If the RPC message is valid and for a procedure implemented by this service, then calls the
    /// procedure implementation.
    ///
    /// Otherwise, returns an error.
    fn handle_received_bytes(&mut self, buffer_id: u16, amount: i32, conn_fd: i32) {
        assert!(amount > 0);

        // SAFETY: the buffer_id was just gotten from a completion.
        let orig_buf = unsafe { self.buffer_map.take_buf(buffer_id) };

        let mut buf = &orig_buf[..amount as usize];

        if buf.len() < 4 {
            // TODO: eventually, this should either try to recv more data, or just submit a
            // cancellation request and close the connection.
            todo!("Not enough bytes to read a record marker. Giving up.");
        }

        let Ok(record_mark) = crate::decode_record_mark(&buf[..4].try_into().unwrap()) else {
            // TODO: either handle this case, or submit a cancellation and close.
            todo!("Not handling message fragments. Giving up");
        };

        buf = &buf[4..]; // Advance buf past the record mark.

        if buf.len() < record_mark as usize {
            // TODO: need to read more data, unfortunately it will come back in anothe buffer, I assume
            todo!("Read was too short. Giving up");
        }

        let call = match decode_call(buf) {
            Ok(call) => call,
            Err(e) => {
                debug!("Protocol error in decoding call: {e}");
                todo!();
            }
        };

        eprintln!("{call:?}");

        let map = &self.procedure_map;
        let Ok(()) =
            validate_program_and_version(&call, map.program, map.version_min, map.version_max)
        else {
            todo!("Handle this");
        };

        let procedure_number = call.get_procedure();
        if procedure_number == 0 {
            todo!("Implement null procedure");
        }

        if procedure_number as usize > map.procedures.len() - 1 {
            debug!("CALL for unknown procedure {}", procedure_number);
            todo!("handle this");
        }

        let Some(procedure) = map.procedures[procedure_number as usize] else {
            debug!("CALL for unimplemented procedure {}", procedure_number);
            todo!("handle this");
        };

        let res = procedure(&call, &mut self.user_state);

        self.process_user_result(res, call.xid, conn_fd);

        // SAFETY: the buffer being resubmitted was just taken at the beginning of this function,
        // and has not been re-submitted before this call.
        unsafe {
            self.buffer_map.resubmit_buf(orig_buf, buffer_id);
        }
    }

    fn process_user_result(&mut self, res: RingResult, xid: u32, conn_fd: i32) {
        match res {
            RingResult::Done(rpc_res) => match rpc_res {
                RpcResult::Success(data) => self.send_succesful_reply(xid, conn_fd, data),
                _ => todo!(),
            },
            RingResult::MoreIo(_) => todo!(),
        }
    }

    fn send_succesful_reply(&mut self, xid: u32, conn_fd: i32, data: Vec<u8>) {
        assert!(conn_fd > 2);
        let buf = encode_succesful_reply(xid, &data);

        let user_data = Send::new(conn_fd, buf);

        let submission =
            opcode::Send::new(types::Fd(conn_fd), user_data.buf_ptr(), user_data.buf_len())
                .build()
                .user_data(Box::new(Operation::Send(user_data)).to_u64());

        // SAFETY: The pointer to the buffer has had its ownership passed to the kernel via
        // `to_u64()`. TODO: need to manage the lifetime of the conn FD, probably with reference
        // counting. This is currently broken.
        unsafe {
            self.ring
                .submission()
                .push(&submission)
                .expect("queue is full");
        }
    }
}

/// Check for fatal errors in completions. These errors always indicate a BUG in this program.
fn check_completion_error(cqe: &cqueue::Entry, op: &Operation) {
    let res = cqe.result();

    // Not an error:
    if res >= 0 {
        return;
    }

    match nix::errno::Errno::from_raw(-res) {
        nix::Error::EBADF => panic!("Completion returned EBADF: {op}, {cqe:?}"),
        nix::Error::EFAULT => panic!("Completion returned EFAULT: {op}, {cqe:?}"),
        _ => {}
    };
}

fn submit_accept(ring: &mut IoUring, listen_fd: types::Fd, user_data: u64) {
    let submission = opcode::AcceptMulti::new(listen_fd)
        .build()
        .user_data(user_data);

    // SAFETY: the parameter listen_fd will be valid for the lifetime of the operation because it
    // is owned by the user_data, which has been "leaked" (passing ownership to the kernel) before
    // calling this function.
    unsafe {
        ring.submission().push(&submission).expect("queue is full");
    }
}

#[derive(Debug)]
enum Operation {
    Accept(Accept),
    Recv(Receive),
    Send(Send),
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Accept(a) => write!(f, "Accept on FD {}", a.fd),
            Self::Recv(r) => write!(f, "Receive on FD {}", r.fd),
            Self::Send(_) => write!(f, "Send"),
        }
    }
}

impl Operation {
    fn handle_accept(self: Box<Self>, ring: &mut IoUring, cqe: cqueue::Entry, listen_fd: i32) {
        let fd = cqe.result();

        if fd < 0 {
            warn!("accept: error: {fd}: {}", io::Error::from_raw_os_error(fd))
        } else {
            let user_data = Box::new(Operation::Recv(Receive::new(fd)));

            let submission = opcode::RecvMulti::new(types::Fd(fd), GROUP_ID)
                .build()
                .user_data(user_data.to_u64());

            unsafe {
                ring.submission().push(&submission).expect("queue is full");
            }
        }

        // Keep submission alive:
        if !cqueue::more(cqe.flags()) {
            warn!("Multishot accept did not set MORE flag; resubmitting");
            submit_accept(ring, types::Fd(listen_fd), self.to_u64_noexpose());
        } else {
            // Leak self again since this submission stays live with self as its user data
            let _ = self.to_u64_noexpose();
        }
    }

    fn handle_receive<T>(
        self: Box<Self>,
        server: &mut RpcServer<T>,
        cqe: cqueue::Entry,
        conn_fd: i32,
    ) {
        match cqe.result() {
            res if res < 0 => {
                warn!("Error in Receive completion: {cqe:?}");
            }
            // Connection is done:
            0 => {
                trace!("Closing connection with fd {conn_fd}");
                // TODO: better resource management of this FD? Does this need reference-counted in
                // case there's an outstanding send on this connection?
                let _ = unsafe { libc::close(conn_fd) };

                // Return early because there is no need to keep this submission alive anymore:
                return;
            }
            // Got data:
            amount => {
                let buffer_id: u16 = cqueue::buffer_select(cqe.flags())
                    .expect("Buffer ID should be set on a multishot receive");

                server.handle_received_bytes(buffer_id, amount, conn_fd);
            }
        }

        // Keep submission alive:
        if !cqueue::more(cqe.flags()) {
            // resubmit receive
            todo!()
        } else {
            // Leak self again since this submission stays live with self as its user data
            let _ = self.to_u64_noexpose();
        }
    }

    /// Temporarily "leak" the Operation so that the kernel side can take ownership of it until the
    /// completion is processed.
    ///
    /// Exposes provenance so that a pointer to the Operation can be acquired with the proper
    /// provenance when processing the completion that holds this data.
    fn to_u64(self: Box<Self>) -> u64 {
        Box::into_raw(self).expose_provenance() as u64
    }

    /// Leak an operation without the need to expose its provenance, because it was already exposed.
    /// Useful when re-submitting multishot requests with the same user_data from the original
    /// submission.
    fn to_u64_noexpose(self: Box<Self>) -> u64 {
        Box::into_raw(self) as u64
    }

    /// Given a u64 which is expected to be a pointer to an Operation, turn it into a
    /// Box<Operation> with some previously exposed provenance.
    ///
    /// SAFETY:
    ///
    /// Uses Box::from_raw() and has the same safety requirements as that function.
    unsafe fn from_u64(p: u64) -> Box<Self> {
        Box::from_raw(std::ptr::with_exposed_provenance::<Operation>(p as usize) as *mut Self)
    }
}

#[derive(Debug)]
struct Accept {
    /// fd for the listener, needed in order to resubmit the accept
    fd: i32,
}

impl Accept {
    fn new(fd: i32) -> Self {
        Self { fd }
    }
}

#[derive(Debug)]
struct Receive {
    fd: i32,
}

impl Receive {
    fn new(fd: i32) -> Self {
        Self { fd }
    }
}

#[derive(Debug)]
struct Send {
    fd: i32,
    data: Vec<u8>,
}

impl Send {
    fn new(fd: i32, data: Vec<u8>) -> Self {
        Self { fd, data }
    }

    fn buf_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    fn buf_len(&self) -> u32 {
        self.data.len() as u32
    }
}

/// A memory map of a ring of buffer descriptors shared with the kernel, along with the buffers
/// themselves.
struct BufferMap {
    /// Pointer to the memory shared with the kernel which holds the `struct io_uring_buf`s. Its
    /// size is `sizeof(struct io_uring_buf) * num_entries`.
    addr: *mut libc::c_void,

    /// The number of entries in the shared buffer ring.
    num_entries: u16,

    /// The size of each buffer.
    _buf_size: u32,

    /// The tail of the ring, including unpublished buffers. This is the index of the next unused
    /// slot.
    private_tail: u16,

    group_id: u16,

    buffers: Vec<Box<[u8]>>,
}

impl BufferMap {
    pub fn new(ring: &mut IoUring) -> Self {
        let num_entries = 1024;
        let buf_size = 4096;

        assert!(num_entries < u16::MAX);
        assert!(num_entries & (num_entries - 1) == 0); // must be a power of 2

        let len = (num_entries as usize) * std::mem::size_of::<types::BufRingEntry>();
        let addr = unsafe {
            match libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_POPULATE,
                -1,
                0,
            ) {
                libc::MAP_FAILED => panic!("mmap: {:?}", io::Error::last_os_error()),
                addr => addr,
            }
        };

        let mut buffer_map = Self {
            addr,
            num_entries,
            _buf_size: buf_size,
            private_tail: 0,
            group_id: GROUP_ID,
            buffers: Vec::new(),
        };

        unsafe {
            ring.submitter()
                .register_buf_ring(buffer_map.addr as u64, num_entries, buffer_map.group_id)
                .unwrap();
        };

        for i in 0..num_entries {
            buffer_map
                .buffers
                .push(vec![0; buf_size as usize].into_boxed_slice());
            let addr: *mut u8 = buffer_map.buffers[i as usize].as_ptr() as *mut u8;
            buffer_map.push_buf(addr, buf_size, i);
        }

        buffer_map.publish_bufs();

        buffer_map
    }

    /// Add a buffer described by `addr`, `len`, and `bid` to the buffer map.
    fn push_buf(&mut self, addr: *mut u8, len: u32, bid: u16) {
        let entries = self.addr as *mut types::BufRingEntry;
        let index: u16 = self.private_tail & self.mask();

        // SAFETY: ...
        let entry = unsafe { entries.add(index as usize) };
        // SAFETY: ...
        let entry = unsafe { &mut *entry };

        entry.set_addr(addr as u64);
        entry.set_len(len);
        entry.set_bid(bid);

        self.private_tail = self.private_tail.wrapping_add(1);
    }

    /// Advance the shared tail to publish new buffers to the kernel.
    fn publish_bufs(&mut self) {
        let base_entry = self.addr as *const types::BufRingEntry;

        // SAFETY: ...
        let shared_tail = unsafe { types::BufRingEntry::tail(base_entry) };
        let shared_tail = shared_tail as *const AtomicU16;

        // SAFETY: ...
        unsafe { (*shared_tail).store(self.private_tail, Ordering::Release) };
    }

    fn mask(&self) -> u16 {
        self.num_entries - 1
    }

    /// SAFETY:
    ///
    /// The caller must ensure that the buffer ID is one returned by the kernel in a completion
    /// event, and which has not been re-submitted to the kernel. Otherwise, reading the buffer can
    /// result in a data race with the kernel writing to that buffer.
    pub unsafe fn take_buf(&mut self, id: u16) -> Box<[u8]> {
        std::mem::take(&mut self.buffers[id as usize])
    }

    /// SAFETY:
    ///
    /// Has the same requirements as take_buf()
    pub unsafe fn borrow_buf(&self, id: u16) -> &[u8] {
        &self.buffers[id as usize]
    }

    /// SAFETY:
    ///
    /// Has the same requirements as take_buf()
    pub unsafe fn resubmit_buf(&mut self, mut buf: Box<[u8]>, id: u16) {
        self.push_buf(buf.as_mut_ptr(), self._buf_size, id);
        self.buffers[id as usize] = buf;
        self.publish_bufs();
    }
}
