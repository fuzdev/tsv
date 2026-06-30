//! Pattern types for public AST (destructuring)

use serde::Serialize;

use super::expressions::Property;
use super::types::TSTypeAnnotation;
use super::{Expression, SourceLocation};

/// Object pattern for destructuring: `{a, b}`
#[derive(Debug, Clone, Serialize)]
pub struct ObjectPattern<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<ObjectPatternProperty<'src>>,
    /// Optional destructuring-pattern parameter (`{a}?`); emitted only when true.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation<'src>>,
}

/// Object pattern property - either a regular property or a rest element
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ObjectPatternProperty<'src> {
    Property(Property<'src>),
    RestElement(RestElement<'src>),
}

/// Array pattern for destructuring: `[a, b]`
#[derive(Debug, Clone, Serialize)]
pub struct ArrayPattern<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub elements: Vec<Option<Expression<'src>>>,
    /// Optional destructuring-pattern parameter (`[a]?`); emitted only when true.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation<'src>>,
}

/// Assignment pattern for default values: `a = 1`
#[derive(Debug, Clone, Serialize)]
pub struct AssignmentPattern<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<Expression<'src>>,
    pub right: Box<Expression<'src>>,
}

/// Rest element in destructuring: `...rest`
#[derive(Debug, Clone, Serialize)]
pub struct RestElement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression<'src>>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation<'src>>,
}
