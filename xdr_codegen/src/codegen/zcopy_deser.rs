use super::*;

impl ValidatedDefinition {
    pub(super) fn definition_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        match self {
            ValidatedDefinition::Const(_) => {
                // We assume definition_copy ran before this
            }
            ValidatedDefinition::Enum(_) => {
                // We assume definition_copy ran before this
            }
            ValidatedDefinition::Struct(s) => {
                s.definition_zcopy(buf, tab);
            }
            ValidatedDefinition::TypeDef(_) => {}
            ValidatedDefinition::Union(_u) => {
                unimplemented!();
            }
        }
    }

    fn as_zcopy_deser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            ValidatedDefinition::Struct(s) => format!("{}Reader<'a>", s.name),
            ValidatedDefinition::Enum(e) => e.name.to_string(),
            ValidatedDefinition::Union(_u) => unimplemented!(),
            ValidatedDefinition::Const(c) => c.value.as_type_name(tab),
            ValidatedDefinition::TypeDef(t) => match &t.decl.kind {
                DeclarationKind::Scalar(ty) => ty.as_zcopy_deser_type_name(tab),
                DeclarationKind::Optional(o) => o.optional_type_name_zcopy(tab),
                DeclarationKind::Array(arr) => arr.as_zcopy_deser_type_name(tab),
            },
        }
    }
}

impl NamedDeclaration {
    fn as_zcopy_dser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.kind {
            DeclarationKind::Scalar(s) => s.as_zcopy_deser_type_name(tab),
            DeclarationKind::Array(arr) => arr.as_zcopy_deser_type_name(tab),
            DeclarationKind::Optional(o) => o.optional_type_name_zcopy(tab),
        }
    }

    pub(super) fn get_size_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,

        fallible_parent: bool,
        member_name: Option<String>,
    ) {
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.get_size_inline_zcopy(buf, tab, fallible_parent, member_name);
            }
            DeclarationKind::Array(a) => {
                a.get_size_inline_zcopy(buf, tab);
            }
            DeclarationKind::Optional(ty) => {
                ty.get_optional_size_inline_zcopy(buf, tab, fallible_parent, member_name);
            }
        }
    }

    pub(super) fn deserialize_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
    ) {
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.deserialize_inline_zcopy(buf, tab, fallible_parent);
            }
            DeclarationKind::Array(a) => {
                a.deserialize_inline_zcopy(buf, tab);
            }
            DeclarationKind::Optional(o) => {
                o.deserialize_optional_inline_zcopy(buf, tab, fallible_parent);
            }
        }
    }

    fn maybe_enum(&self, tab: &ValidatedSymbolTable) -> Option<ValidatedEnum> {
        match &self.kind {
            DeclarationKind::Scalar(XdrType::Name(n)) => {
                let val = tab.lookup_definition(n);
                match val {
                    ValidatedDefinition::TypeDef(xdr_type_def) => xdr_type_def.decl.maybe_enum(tab),
                    ValidatedDefinition::Enum(validated_enum) => Some(validated_enum.clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl XdrType {
    pub(super) fn as_zcopy_deser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            XdrType::Int => "i32".to_string(),
            XdrType::UInt => "u32".to_string(),
            XdrType::Hyper => "i64".to_string(),
            XdrType::UHyper => "u64".to_string(),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => "bool".to_string(),
            XdrType::Name(s) => tab.lookup_definition(s).as_zcopy_deser_type_name(tab),
        }
    }

    pub(super) fn get_size_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,

        fallible_parent: bool,
        member_name: Option<String>,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl
                    .get_size_inline_zcopy(buf, tab, fallible_parent, member_name);
                return;
            };
        };

        let unwrap_method = if fallible_parent { "?" } else { ".unwrap()" };

        if let Some(found_size) = self.size(tab) {
            buf.add_line(&format!("Ok({})", found_size));
        } else {
            // typedef case already handled, non-typedefs follow:
            let (method, fallible) = self.deserialize_method_zcopy(tab);

            if let Some(cache_name) = member_name {
                buf.block_statement(
                    &format!("if let Some(cached) = {}.get()", cache_name),
                    |buf| {
                        buf.add_line("return cached.get_width();");
                    },
                );
                buf.add_line(&format!(
                    "let val = {method}(_input){};",
                    if fallible { unwrap_method } else { "" }
                ));
                buf.add_line(&format!("{cache_name}.get_or_init(|| val).get_width()"));
            } else {
                buf.add_line(&format!(
                    "{method}(_input){}.get_width()",
                    if fallible { unwrap_method } else { "" }
                ));
            }
        }
    }

    pub(super) fn get_optional_size_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
        reader_name: Option<String>,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl
                    .get_size_inline_zcopy(buf, tab, fallible_parent, reader_name);
                return;
            };
        };

        buf.add_line("let has_optional = xdr_lib::get_i32_infallible(_input);");
        buf.code_block("match has_optional", |buf| {
            buf.add_line("0 => Ok(4),");
            buf.code_block("_ =>", |buf| {
                if self.self_referential_optional(tab) {
                    buf.add_line(&format!(
                        "let mut it = xdr_lib::LinkedListIter::<'a, {}>::new(_input, {});",
                        self.as_zcopy_deser_type_name(tab),
                        self.size(tab)
                            .map(|v| format!("Some({})", v))
                            .unwrap_or("None".to_string())
                    ));

                    buf.add_line("it.by_ref().for_each(drop);");
                    buf.add_line("");
                    buf.add_line("Ok(it.off)");
                } else if let Some(opt_size) = self.size(tab) {
                    buf.add_line(&format!("Ok({})", opt_size + 4));
                } else {
                    buf.add_line("let off = off + 4;");
                    buf.add_line("let _input = &self.buf[off..];");
                    self.get_size_inline_zcopy(buf, tab, fallible_parent, reader_name.clone());
                    buf.add_line("\t.map(|val| val + 4usize)");
                }
            });
        });
    }

    pub(super) fn deserialize_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
    ) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl
                    .deserialize_inline_zcopy(buf, tab, fallible_parent);
                return;
            };
        };

        let unwrap_method = if fallible_parent { "?" } else { ".unwrap()" };

        // typedef case already handled, non-typedefs follow:
        let (method, fallible) = self.deserialize_method_zcopy(tab);
        if !fallible {
            buf.add_line(&format!("{method}(_input)"));
        } else {
            buf.add_line(&format!("{method}(_input){unwrap_method}"));
        }
    }

    pub(super) fn deserialize_optional_inline_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
        fallible_parent: bool,
    ) {
        if self.self_referential_optional(tab) {
            buf.add_line(&format!(
                "xdr_lib::LinkedListIter::<'a, {}>::new(_input, {})",
                self.as_zcopy_deser_type_name(tab),
                self.size(tab)
                    .map(|v| format!("Some({})", v))
                    .unwrap_or("None".to_string())
            ));
        } else {
            buf.add_line("let has_val = xdr_lib::get_i32_infallible(_input);");
            buf.code_block("match has_val", |buf| {
                buf.add_line("0 => None,");
                buf.code_block("_ =>", |buf| {
                    buf.block_statement("let val = ", |buf| {
                        buf.add_line("let off = off + 4;");
                        buf.add_line("let _input = &self.buf[off..];");
                        self.deserialize_inline_zcopy(buf, tab, fallible_parent);
                    });
                    buf.add_line("Some(val)");
                    // buf.add_line("val.map(|v| Some(v))");
                })
            });
        }
    }

    // returns (name, fallible)
    fn deserialize_method_zcopy(&self, tab: &ValidatedSymbolTable) -> (String, bool) {
        match self {
            XdrType::Int => ("xdr_lib::get_i32_infallible".to_string(), false),
            XdrType::UInt => ("xdr_lib::get_u32_infallible".to_string(), false),
            XdrType::Hyper => ("xdr_lib::get_i64_infallible".to_string(), false),
            XdrType::UHyper => ("xdr_lib::get_u64_infallible".to_string(), false),
            XdrType::Float => todo!(),
            XdrType::Double => todo!(),
            XdrType::Quadruple => todo!(),
            XdrType::Bool => ("xdr_lib::get_bool_infallible".to_string(), false),
            XdrType::Name(n) => {
                if let ValidatedDefinition::Enum(_) = tab.lookup_definition(n) {
                    (format!("{n}::deserialize"), true)
                } else {
                    (format!("{n}Reader::from_buf"), true)
                }
            }
        }
    }
}

impl ValidatedStruct {
    fn definition_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        // let deps = self.get_variable_width_last_deps();
        let (deps, self_ref_last) = if let Some(last) = self.members.last() {
            if self.member_is_self_referential(&last.0, tab) {
                (self.get_variable_width_last_deps(), Some(&last.0))
            } else {
                (self.get_variable_width_members(tab), None)
            }
        } else {
            (self.get_variable_width_last_deps(), None)
        };

        buf.add_line("#[derive(Debug, PartialEq, Clone, Default)]");
        buf.code_block(&format!("pub struct {}Reader <'a>", self.name), |buf| {
            buf.add_line("buf: &'a [u8],");
            for dep in deps.iter() {
                let (member, _) = self.members.iter().find(|v| v.0.name == **dep).unwrap();
                if member.is_varlen_reader(tab) {
                    let typename = member.as_zcopy_dser_type_name(tab);

                    buf.add_line(&format!("{}: {},", dep, typename));
                } else {
                    buf.add_line(&format!("{}_width: usize,", dep));
                }
            }

            if let Some(last) = self_ref_last {
                buf.add_line(&format!("{}_width: std::cell::OnceCell<usize>,", last.name));
            }
        });

        buf.add_line("");
        buf.code_block(
            &format!("impl<'a> xdr_lib::Reader<'a> for {}Reader <'a>", &self.name),
            |buf| {
                buf.code_block(
                    "fn from_buf(buf: &'a [u8]) -> xdr_lib::Result<Self>",
                    |buf| {
                        let deps_in_order = self.get_variable_width_members_ordered(tab);
                        for (nd, off) in deps_in_order.iter() {
                            if self.member_is_self_referential(nd, tab) {
                                buf.add_line(&format!("let {}_width = std::cell::OnceCell::<usize>::new();", nd.name));
                                continue;
                            }

                            buf.add_line(&format!(
                                "let off = {};",
                                Self::offset_to_string_localvars(off)
                            ));
                            buf.add_line("let _input = &buf[off..];");
                            if nd.is_varlen_reader(tab) {
                                    let typename = nd.as_zcopy_dser_type_name(tab);
                                    let typename = typename.strip_suffix("<'a>").map(|rest| format!("{}::<'a>", rest)).unwrap_or(typename.to_string());
                                    let typename = typename.strip_prefix("Option").map(|rest| format!("Option::{}", rest)).unwrap_or(typename.to_string());

                                    buf.add_line(&format!("let {} = {}::from_buf(&buf[off..])?;", nd.name, typename));
                                    buf.add_line(&format!(
                                        "let {}_width = {}.get_width()?;",
                                        nd.name, nd.name
                                    ));
                            } else {
                                buf.block_with_trailer(
                                    &format!("let {}_width = ", nd.name),
                                    "?;",
                                    |buf| {
                                        match &nd.kind {
                                            DeclarationKind::Scalar(xdr_type) => match xdr_type {
                                                XdrType::Name(_) => {
                                                    xdr_type.get_size_inline_zcopy(
                                                        buf, tab, true, None,
                                                    );
                                                }
                                                _ => unreachable!("we should only have indeterminate named types here"),
                                            },
                                            DeclarationKind::Array(array) => {

                                                array.get_size_inline_zcopy(buf, tab);
                                            }
                                            DeclarationKind::Optional(xdr_type) => {
                                xdr_type.get_optional_size_inline_zcopy(
                                    buf,
                                    tab,
                                    true,
                                    None
                                );
                                            }
                                        };
                                    },
                                );
                            }

                            buf.add_line("");
                        }

                        buf.block_statement("let me = Self", |buf| {
                            buf.add_line("buf,");
                            for (nd, _) in deps_in_order.iter() {
                                if self.member_is_self_referential(nd, tab) {
                                    buf.add_line(&format!("{}_width,", nd.name));
                                    continue;
                                }

                                if nd.is_varlen_reader(tab) {
                                    buf.add_line(&format!("{},", nd.name));
                                } else {
                                    buf.add_line(&format!("{}_width,", nd.name));
                                }
                            }
                        });

                        buf.add_line("me.validate()");
                    },
                );

                buf.code_block("fn get_width(&self) -> xdr_lib::Result<usize>", |buf| {
                    if let Some((last, last_off)) = self.members.last() {
                        let last_size = last.size(tab);
                        let mut overall_definition_size = DefinitionSize {
                            known: last_off.known + last_size.unwrap_or(0),
                            deps: last_off.deps.clone(),
                        };

                        if last_size.is_none() {
                            overall_definition_size.deps.push(last.name.clone());
                        }

                        buf.add_line(&format!(
                            "Ok({})",
                            &Self::offset_to_string(&overall_definition_size)
                        ));
                    } else {
                        buf.add_line("Ok(0)");
                    }
                });
            },
        );
    }

    pub(super) fn deserialize_definition_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        buf.code_block("fn validate(self) -> xdr_lib::Result<Self>", |buf| {
            if let Some((last, last_off)) = self.members.last() {
                let last_size = if self.member_is_self_referential(last, tab) {
                    // We skip last members who evaluate to iterators, which could be a
                    // large performance sink
                    Some(0)
                } else {
                    last.size(tab)
                };

                let mut overall_definition_size = DefinitionSize {
                    known: last_off.known + last_size.unwrap_or(0),
                    deps: last_off.deps.clone(),
                };

                if last_size.is_none() {
                    overall_definition_size.deps.push(last.name.clone());
                }

                buf.add_line(&format!(
                    "let required = {};",
                    &Self::offset_to_string(&overall_definition_size)
                ));
                buf.code_block("if required > self.buf.len()", |buf| {
                    buf.add_line("return Err(xdr_lib::DeserializeError);");
                });
            }

            // Validate enums on struct creation
            for ((nd, off), _) in self
                .members
                .iter()
                .filter_map(|v| v.0.maybe_enum(tab).map(|en| (v, en)))
            {
                buf.add_line(&format!("let off = {};", Self::offset_to_string(off)));
                buf.add_line("let _input = &self.buf[off..];");
                match &nd.kind {
                    DeclarationKind::Scalar(xdr_type) => buf.add_line(&format!(
                        "{}(_input)?;",
                        xdr_type.deserialize_method_zcopy(tab).0
                    )),
                    _ => unreachable!(),
                }
            }

            buf.add_line("Ok(self)");
        });

        let deps = self.get_variable_width_members(tab);

        for dep in deps.iter() {
            buf.code_block(
                &format!("pub fn get_{}_width(&self) -> xdr_lib::Result<usize>", dep),
                |buf| {
                    let (member, member_off) = self
                        .members
                        .iter()
                        .find(|(nd, _)| nd.name == **dep)
                        .unwrap();

                    if self.member_is_self_referential(member, tab) {
                        match &member.kind {
                            DeclarationKind::Optional(xdr_type)
                            | DeclarationKind::Scalar(xdr_type) => {
                                buf.add_line(&format!(
                                    "let off = {};",
                                    Self::offset_to_string(member_off)
                                ));
                                buf.add_line("let _input = &self.buf[off..];");
                                xdr_type.get_optional_size_inline_zcopy(buf, tab, true, None);
                            }
                            _ => unreachable!(),
                        };
                    } else {
                        if member.is_varlen_reader(tab) {
                            buf.add_line(&format!("self.{}.get_width()", member.name));
                        } else {
                            buf.add_line(&format!("Ok(self.{}_width)", member.name));
                        }
                    }
                },
            );
        }

        for (member, member_off) in self.members.iter() {
            buf.code_block(
                &format!(
                    "pub fn get_{}(&self) -> {}",
                    member.name,
                    member.as_zcopy_dser_type_name(tab)
                ),
                |buf| {
                    if member.size(tab).is_none() && member.is_varlen_reader(tab) {
                        buf.add_line(&format!("return self.{}.clone()", member.name));
                        return;
                    }

                    if member_off.is_determinate() {
                        buf.add_line(&format!("let off = {};", member_off.known));
                    } else {
                        buf.add_line(&format!(
                            "let off = {};",
                            Self::offset_to_string_infallible(member_off)
                        ));
                    }

                    buf.add_line("let _input = &self.buf[off..];");

                    // Validation can be here
                    member.deserialize_inline_zcopy(buf, tab, false);
                },
            );
        }
    }
}

impl ValidatedEnum {
    pub(super) fn deserialize_definition_zcopy(
        &self,
        buf: &mut CodeBuf,
        tab: &ValidatedSymbolTable,
    ) {
        buf.code_block(
            "pub fn deserialize(_input: &[u8]) -> xdr_lib::Result<Self>",
            |buf| {
                buf.add_line("let val = xdr_lib::get_i32_infallible(_input);");
                buf.code_block("match val", |buf| {
                    for variant in self.variants.iter() {
                        let val = variant.1.as_const(tab);
                        buf.add_line(&format!("{} => Ok({}::{}),", val, self.name, variant.0));
                    }
                    buf.add_line("_ => Err(xdr_lib::DeserializeError),");
                });
            },
        );

        buf.add_line("pub fn get_width(&self) -> usize {4}");
    }
}

impl Array {
    pub(super) fn as_zcopy_deser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.kind {
            ArrayKind::Ascii => "&'a std::ffi::OsStr".to_string(),
            ArrayKind::Byte => "&'a [u8]".to_string(),
            ArrayKind::UserType(ty) => {
                format!(
                    "xdr_lib::ArrayIter<'a, {}>",
                    ty.as_zcopy_deser_type_name(tab)
                )
            }
        }
    }

    pub(super) fn zcopy_gen_inner_type(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.kind {
            ArrayKind::Ascii | ArrayKind::Byte => "u8".to_string(),
            ArrayKind::UserType(ty) => ty.as_zcopy_deser_type_name(tab),
        }
    }

    pub(super) fn get_size_inline_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.add_line(&self.array_count_extractor(Some("length"), tab, false, true));

        if let Some(elem_width) = self.elem_size(tab) {
            buf.add_line(&format!(
                "let required = xdr_lib::geq_4byte_boundary(length * {}usize) + _array_count_size;",
                elem_width
            ));

            buf.add_line("Ok(required)");
        } else {
            buf.add_line(&format!(
                "let mut it = xdr_lib::ArrayIter::<'a, {}>::new(_input, length, None);",
                self.zcopy_gen_inner_type(tab)
            ));

            buf.add_line("it.by_ref().for_each(drop);");
            buf.add_line("");
            buf.code_block("if it.i != length", |buf| {
                buf.add_line("return Err(xdr_lib::DeserializeError);");
            });
            buf.add_line("");
            buf.add_line("Ok(it.off + _array_count_size)");
        }
    }

    pub(super) fn deserialize_inline_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        buf.add_line(&self.array_count_extractor(Some("length"), tab, true, false));

        match &self.kind {
            ArrayKind::Byte => buf.add_line("&_input[..length]"),
            ArrayKind::Ascii => buf.add_line("std::ffi::OsStr::from_bytes(&_input[..length])"),
            ArrayKind::UserType(_) => {
                buf.add_line(&format!(
                    "xdr_lib::ArrayIter::<'a, {}>::new(_input, length, None)",
                    self.zcopy_gen_inner_type(tab)
                ));
            }
        }
    }
}
