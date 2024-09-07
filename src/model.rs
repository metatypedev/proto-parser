use std::fmt;
use std::fmt::Write;

use std::ops::{Deref, RangeInclusive};

use indexmap::IndexMap;
use protobuf::reflect::{ReflectValueBox, RuntimeType};

use crate::{
    lexer::{float::format_protobuf_float, loc::Loc, strlit::StrLit},
    proto_path::ProtoPathBuf,
    protobuf_abs_path::ProtobufAbsPath,
    protobuf_ident::ProtobufIdent,
    protobuf_path::ProtobufPath,
    Parser, ParserErrorWithLocation, Syntax,
};

#[derive(thiserror::Error, Debug)]
enum ModelError {
    #[error("cannot convert value `{1}` to type `{0}`")]
    InconvertibleValue(RuntimeType, ProtobufConstant),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithLoc<T> {
    pub loc: Loc,
    pub t: T,
}

impl<T> Deref for WithLoc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.t
    }
}

impl<T> WithLoc<T> {
    pub fn with_loc(loc: Loc) -> impl FnOnce(T) -> WithLoc<T> {
        move |t| WithLoc { t, loc }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProtobufOption {
    pub name: ProtobufOptionName,
    pub value: ProtobufConstant,
}

/// Visibility of import statement
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ImportVis {
    Default,
    Public,
    Weak,
}

impl Default for ImportVis {
    fn default() -> Self {
        ImportVis::Default
    }
}

#[derive(Debug, Default, Clone)]
pub struct Import {
    pub path: ProtoPathBuf,
    pub vis: ImportVis,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AnyTypeUrl {
    pub(crate) prefix: String,
    pub(crate) full_type_name: ProtobufPath,
}

impl fmt::Display for AnyTypeUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.prefix, self.full_type_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ProtobufConstantMessageFieldName {
    Regular(String),
    Extension(ProtobufPath),
    AnyTypeUrl(AnyTypeUrl),
}

impl fmt::Display for ProtobufConstantMessageFieldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtobufConstantMessageFieldName::Regular(s) => write!(f, "{}", s),
            ProtobufConstantMessageFieldName::Extension(p) => write!(f, "[{}]", p),
            ProtobufConstantMessageFieldName::AnyTypeUrl(a) => write!(f, "[{}]", a),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProtobufConstantMessage {
    pub(crate) fields: IndexMap<ProtobufConstantMessageFieldName, ProtobufConstant>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProtobufConstant {
    U64(u64),
    I64(i64),
    F64(f64), // TODO: eq
    Bool(bool),
    Ident(ProtobufPath),
    String(StrLit),
    Message(ProtobufConstantMessage),
}

impl fmt::Display for ProtobufConstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtobufConstant::U64(v) => write!(f, "{}", v),
            ProtobufConstant::I64(v) => write!(f, "{}", v),
            ProtobufConstant::F64(v) => write!(f, "{}", format_protobuf_float(*v)),
            ProtobufConstant::Bool(v) => write!(f, "{}", v),
            ProtobufConstant::Ident(v) => write!(f, "{}", v),
            ProtobufConstant::String(v) => write!(f, "{}", v),
            // TODO: text format explicitly
            ProtobufConstant::Message(v) => write!(f, "{:?}", v),
        }
    }
}

impl ProtobufConstantMessage {
    pub fn format(&self) -> String {
        let mut s = String::new();
        write!(s, "{{").unwrap();
        for (n, v) in &self.fields {
            match v {
                ProtobufConstant::Message(m) => write!(s, "{} {}", n, m.format()).unwrap(),
                v => write!(s, "{}: {}", n, v.format()).unwrap(),
            }
        }
        write!(s, "}}").unwrap();
        s
    }
}

impl ProtobufConstant {
    pub fn format(&self) -> String {
        match *self {
            ProtobufConstant::U64(u) => u.to_string(),
            ProtobufConstant::I64(i) => i.to_string(),
            ProtobufConstant::F64(f) => format_protobuf_float(f),
            ProtobufConstant::Bool(b) => b.to_string(),
            ProtobufConstant::Ident(ref i) => format!("{}", i),
            ProtobufConstant::String(ref s) => s.quoted(),
            ProtobufConstant::Message(ref s) => s.format(),
        }
    }

    /** Interpret .proto constant as an reflection value. */
    pub fn as_type(&self, ty: RuntimeType) -> anyhow::Result<ReflectValueBox> {
        match (self, &ty) {
            (ProtobufConstant::Ident(ident), RuntimeType::Enum(e)) => {
                if let Some(v) = e.value_by_name(&ident.to_string()) {
                    return Ok(ReflectValueBox::Enum(e.clone(), v.value()));
                }
            }
            (ProtobufConstant::Bool(b), RuntimeType::Bool) => return Ok(ReflectValueBox::Bool(*b)),
            (ProtobufConstant::String(lit), RuntimeType::String) => {
                return Ok(ReflectValueBox::String(lit.decode_utf8()?))
            }
            _ => {}
        }
        Err(ModelError::InconvertibleValue(ty.clone(), self.clone()).into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProtobufOptionNamePart {
    Direct(ProtobufIdent),
    Ext(ProtobufPath),
}

impl fmt::Display for ProtobufOptionNamePart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtobufOptionNamePart::Direct(n) => write!(f, "{}", n),
            ProtobufOptionNamePart::Ext(n) => write!(f, "({})", n),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProtobufOptionNameExt(pub Vec<ProtobufOptionNamePart>);

#[derive(Debug, Clone, PartialEq)]
pub enum ProtobufOptionName {
    Builtin(ProtobufIdent),
    Ext(ProtobufOptionNameExt),
}

impl ProtobufOptionName {
    pub fn simple(name: &str) -> ProtobufOptionName {
        ProtobufOptionName::Builtin(ProtobufIdent::new(name))
    }
}

impl fmt::Display for ProtobufOptionNameExt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, comp) in self.0.iter().enumerate() {
            if index != 0 {
                write!(f, ".")?;
            }
            write!(f, "{}", comp)?;
        }
        Ok(())
    }
}

impl fmt::Display for ProtobufOptionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtobufOptionName::Builtin(n) => write!(f, "{}", n),
            ProtobufOptionName::Ext(n) => write!(f, "{}", n),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Rule {
    Optional,
    Repeated,
    Required,
}

impl Rule {
    pub(crate) const ALL: [Rule; 3] = [Rule::Optional, Rule::Repeated, Rule::Required];

    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Rule::Optional => "optional",
            Rule::Repeated => "repeated",
            Rule::Required => "required",
        }
    }
}

/// A Protobuf Field
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Field name
    pub name: String,
    /// Field `Rule`
    pub rule: Option<Rule>,
    /// Field type
    pub typ: FieldType,
    /// Tag number
    pub number: i32,
    /// Non-builtin options
    pub options: Vec<ProtobufOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub fields: Vec<WithLoc<Field>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Bool,
    Fixed64,
    Sfixed64,
    Double,
    String,
    Bytes,
    Fixed32,
    Sfixed32,
    Float,
    MessageOrEnum(ProtobufPath),
    Map(Box<(FieldType, FieldType)>),
    Group(Group),
}

#[derive(Debug, Clone, Default)]
pub struct Message {
    /// Message name
    pub name: String,
    /// Message fields and oneofs
    pub fields: Vec<WithLoc<FieldOrOneOf>>,
    /// Message reserved numbers
    pub reserved_nums: Vec<RangeInclusive<i32>>,
    /// Message reserved names
    pub reserved_names: Vec<String>,
    /// Nested messages
    pub messages: Vec<WithLoc<Message>>,
    /// Nested enums
    pub enums: Vec<WithLoc<Enumeration>>,
    /// Non-builtin options
    pub options: Vec<ProtobufOption>,
    /// Extension field numbers
    pub extension_ranges: Vec<RangeInclusive<i32>>,
    /// Extensions
    pub extensions: Vec<WithLoc<Extension>>,
}

impl Message {
    pub fn regular_fields_including_in_oneofs(&self) -> Vec<&WithLoc<Field>> {
        self.fields
            .iter()
            .flat_map(|fo| match &fo.t {
                FieldOrOneOf::Field(f) => vec![f],
                FieldOrOneOf::OneOf(o) => o.fields.iter().collect(),
            })
            .collect()
    }

    /** Find a field by name. */
    pub fn field_by_name(&self, name: &str) -> Option<&Field> {
        self.regular_fields_including_in_oneofs()
            .iter()
            .find(|f| f.t.name == name)
            .map(|f| &f.t)
    }

    pub fn _nested_extensions(&self) -> Vec<&Group> {
        self.regular_fields_including_in_oneofs()
            .into_iter()
            .flat_map(|f| match &f.t.typ {
                FieldType::Group(g) => Some(g),
                _ => None,
            })
            .collect()
    }

    #[cfg(test)]
    pub fn regular_fields_for_test(&self) -> Vec<&Field> {
        self.fields
            .iter()
            .flat_map(|fo| match &fo.t {
                FieldOrOneOf::Field(f) => Some(&f.t),
                FieldOrOneOf::OneOf(_) => None,
            })
            .collect()
    }

    pub(crate) fn oneofs(&self) -> Vec<&OneOf> {
        self.fields
            .iter()
            .flat_map(|fo| match &fo.t {
                FieldOrOneOf::Field(_) => None,
                FieldOrOneOf::OneOf(o) => Some(o),
            })
            .collect()
    }
}

/// A protobuf enumeration field
#[derive(Debug, Clone)]
pub struct EnumValue {
    /// enum value name
    pub name: String,
    /// enum value number
    pub number: i32,
    /// enum value options
    pub options: Vec<ProtobufOption>,
}

/// A protobuf enumerator
#[derive(Debug, Clone)]
pub struct Enumeration {
    /// enum name
    pub name: String,
    /// enum values
    pub values: Vec<EnumValue>,
    /// enum options
    pub options: Vec<ProtobufOption>,
    /// enum reserved numbers
    pub reserved_nums: Vec<RangeInclusive<i32>>,
    /// enum reserved names
    pub reserved_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Extension {
    /// Extend this type with field
    pub extendee: ProtobufPath,
    /// Extension field
    pub field: WithLoc<Field>,
}

/// Service method
#[derive(Debug, Clone)]
pub struct Method {
    /// Method name
    pub name: String,
    /// Input type
    pub input_type: ProtobufPath,
    /// Output type
    pub output_type: ProtobufPath,
    /// If this method is client streaming
    #[allow(dead_code)] // TODO
    pub client_streaming: bool,
    /// If this method is server streaming
    #[allow(dead_code)] // TODO
    pub server_streaming: bool,
    /// Method options
    pub options: Vec<ProtobufOption>,
}

/// Service definition
#[derive(Debug, Clone)]
pub struct Service {
    /// Service name
    pub name: String,
    pub methods: Vec<Method>,
    pub options: Vec<ProtobufOption>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OneOf {
    /// OneOf name
    pub name: String,
    /// OneOf fields
    pub fields: Vec<WithLoc<Field>>,
    /// oneof options
    pub options: Vec<ProtobufOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldOrOneOf {
    Field(WithLoc<Field>),
    OneOf(OneOf),
}

/// A File descriptor representing a whole .proto file
#[derive(Debug, Default, Clone)]
pub struct FileDescriptor {
    /// Imports
    pub imports: Vec<Import>,
    /// Package
    pub package: ProtobufAbsPath,
    /// Protobuf Syntax
    pub syntax: Syntax,
    /// Top level messages
    pub messages: Vec<WithLoc<Message>>,
    /// Enums
    pub enums: Vec<WithLoc<Enumeration>>,
    /// Extensions
    pub extensions: Vec<WithLoc<Extension>>,
    /// Services
    pub services: Vec<WithLoc<Service>>,
    /// Non-builtin options
    pub options: Vec<ProtobufOption>,
}

impl FileDescriptor {
    /// Parses a .proto file content into a `FileDescriptor`
    pub fn parse<S: AsRef<str>>(file: S) -> Result<Self, ParserErrorWithLocation> {
        let mut parser = Parser::new(file.as_ref());
        match parser.next_proto() {
            Ok(r) => Ok(r),
            Err(error) => {
                let Loc { line, col } = parser.tokenizer.loc();
                Err(ParserErrorWithLocation { error, line, col })
            }
        }
    }
}
