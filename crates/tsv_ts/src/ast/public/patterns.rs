//! Pattern types for public AST (destructuring)

use serde::Serialize;

use super::expressions::Property;
use super::types::TSTypeAnnotation;
use super::{Expression, SourceLocation};

/// Object pattern for destructuring: `{a, b}`
#[derive(Debug, Clone, Serialize)]
pub struct ObjectPattern {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub properties: Vec<ObjectPatternProperty>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

/// Object pattern property - either a regular property or a rest element
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ObjectPatternProperty {
    Property(Property),
    RestElement(RestElement),
}

/// Array pattern for destructuring: `[a, b]`
#[derive(Debug, Clone, Serialize)]
pub struct ArrayPattern {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub elements: Vec<Option<Expression>>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}

/// Assignment pattern for default values: `a = 1`
#[derive(Debug, Clone, Serialize)]
pub struct AssignmentPattern {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
}

/// Rest element in destructuring: `...rest`
#[derive(Debug, Clone, Serialize)]
pub struct RestElement {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub loc: SourceLocation,
    pub argument: Box<Expression>,
    #[serde(rename = "typeAnnotation", skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TSTypeAnnotation>,
}
