use super::*;

impl ValidatedDefinition {
    pub(super) fn definition_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        match self {
            ValidatedDefinition::Const(c) => {
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

    pub(super) fn as_zcopy_deser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match self {
            ValidatedDefinition::Struct(s) => format!("{}Reader<'a>", s.name),
            ValidatedDefinition::Enum(e) => e.name.to_string(),
            ValidatedDefinition::Union(_u) => unimplemented!(),
            ValidatedDefinition::Const(c) => c.value.as_type_name(tab),
            ValidatedDefinition::TypeDef(t) => match &t.decl.kind {
                DeclarationKind::Scalar(ty) => ty.as_zcopy_deser_type_name(tab),
                DeclarationKind::Optional(_o) => unimplemented!(),
                DeclarationKind::Array(_arr) => unimplemented!(),
            },
        }
    }
}

impl NamedDeclaration {
    pub(super) fn as_zcopy_dser_type_name(&self, tab: &ValidatedSymbolTable) -> String {
        match &self.kind {
            DeclarationKind::Scalar(s) => s.as_zcopy_deser_type_name(tab),
            DeclarationKind::Optional(_o) => unimplemented!(),
            DeclarationKind::Array(_arr) => unimplemented!(),
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
            DeclarationKind::Array(_a) => {
                unimplemented!();
            }
            DeclarationKind::Optional(_ty) => {
                unimplemented!();
            }
        }
    }

    pub(super) fn deserialize_inline_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        match &self.kind {
            DeclarationKind::Scalar(ty) => {
                ty.deserialize_inline_zcopy(buf, tab);
            }
            DeclarationKind::Array(_a) => {
                unimplemented!();
            }
            DeclarationKind::Optional(_o) => {
                unimplemented!();
            }
        }
    }

    fn maybe_enum(&self, tab: &ValidatedSymbolTable) -> Option<ValidatedEnum> {
        match &self.kind {
            DeclarationKind::Scalar(xdr_type) => match xdr_type {
                XdrType::Name(n) => {
                    let val = tab.lookup_definition(n);
                    match val {
                        ValidatedDefinition::TypeDef(xdr_type_def) => {
                            xdr_type_def.decl.maybe_enum(tab)
                        }
                        ValidatedDefinition::Enum(validated_enum) => Some(validated_enum.clone()),
                        _ => None,
                    }
                }
                _ => None,
            },
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

    pub(super) fn deserialize_inline_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        // Handle typedefs specially by finding their underlying type:
        if let XdrType::Name(name) = self {
            let definition = tab.lookup_definition(name);
            if let ValidatedDefinition::TypeDef(ref tdef) = *definition {
                tdef.decl.deserialize_inline_zcopy(buf, tab);
                return;
            };
        };

        // typedef case already handled, non-typedefs follow:
        let (method, fallible) = self.deserialize_method_zcopy(tab);
        if !fallible {
            buf.add_line(&format!("{method}(_input)"));
        } else {
            buf.add_line(&format!("{method}(_input).unwrap()"));
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
    pub(super) fn definition_zcopy(&self, buf: &mut CodeBuf, tab: &ValidatedSymbolTable) {
        let deps = self.get_variable_width_members(tab);

        buf.add_line("#[derive(Debug, PartialEq, Clone, Default)]");
        buf.code_block(&format!("pub struct {}Reader <'a>", self.name), |buf| {
            buf.add_line("buf: &'a [u8],");
            for dep in deps.iter() {
                buf.add_line(&format!("{}_width: usize,", dep));

                let (member, _) = self.members.iter().find(|v| v.0.name == **dep).unwrap();
                if member.is_varlen_reader(tab) {
                    let typename = member.as_zcopy_dser_type_name(tab);

                    buf.add_line(&format!("{}: {},", dep, typename));
                }
            }
        });

        buf.add_line("");
        buf.code_block(
            &format!("impl<'a> xdr_lib::Reader<'a> for {}Reader <'a>", &self.name),
            |buf| {
                buf.code_block(
                    "fn from_buf(buf: &'a [u8]) -> xdr_lib::Result<Self>",
                    |buf| {
                        buf.add_line("let me = Self{ buf,..Default::default() };");
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
                buf.add_line(&format!("let _input = &self.buf[off..];"));
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

                    buf.block_statement(
                        &format!("if let Some(width) = self.{}_width.get()", member.name),
                        |buf| buf.add_line("return Ok(width.clone());"),
                    );

                    buf.block_statement("let width = ", |buf| {
                        match &member.kind {
                            DeclarationKind::Scalar(xdr_type) => match xdr_type {
                                XdrType::Name(_) => {
                                    buf.add_line(&format!(
                                        "let off = {};",
                                        Self::offset_to_string(member_off)
                                    ));
                                    buf.add_line("let _input = &self.buf[off..];");
                                    xdr_type.get_size_inline_zcopy(
                                        buf,
                                        tab,
                                        true,
                                        Some(format!("self.{}", member.name)),
                                    );
                                }
                                _ => unreachable!(
                                    "we should only have indeterminate named types here"
                                ),
                            },
                            DeclarationKind::Array(_array) => unimplemented!(),
                            DeclarationKind::Optional(_xdr_type) => unimplemented!(),
                        };
                    });

                    buf.add_line(&format!(
                        "let _ = self.{}_width.set(width.clone()?);",
                        member.name
                    ));
                    buf.add_line("width.clone()");
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
                    if member.size(tab).is_none() {
                        if member.is_varlen_reader(tab) {
                            buf.add_line(&format!("return self.{}.clone()", member.name));
                            return;
                        }
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
                    member.deserialize_inline_zcopy(buf, tab);
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
