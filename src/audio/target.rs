#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetId(String);
impl From<String> for TargetId {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl From<&str> for TargetId {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub id: TargetId,
    pub display_label: String,
    pub label: String,
}
impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}
