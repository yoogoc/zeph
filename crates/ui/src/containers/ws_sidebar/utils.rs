use gpui::SharedString;
use uuid::Uuid;

pub const SERVICE_ID: &str = "services";
pub const SCHEMA_ID: &str = "schemas";

#[derive(Debug)]
pub enum TreeId {
    ServiceRoot,
    SchemaRoot,
    ServiceFolder(Uuid),
    SchemaFolder(Uuid),
    Service(Uuid),
    Schema(Uuid),
}

impl TreeId {
    pub fn new(value: SharedString) -> Self {
        let parts: Vec<&str> = value.split("/").collect();
        let x = parts[0];
        if parts.len() == 1 {
            match x {
                SERVICE_ID => TreeId::ServiceRoot,
                SCHEMA_ID => TreeId::SchemaRoot,
                _ => unreachable!(),
            }
        } else if parts.len() == 2 {
            match x {
                SERVICE_ID => TreeId::Service(Uuid::parse_str(&parts[1]).unwrap()),
                SCHEMA_ID => TreeId::Schema(Uuid::parse_str(&parts[1]).unwrap()),
                _ => unreachable!(),
            }
        } else if parts.len() == 3 {
            match x {
                SERVICE_ID => TreeId::ServiceFolder(Uuid::parse_str(&parts[2]).unwrap()),
                SCHEMA_ID => TreeId::SchemaFolder(Uuid::parse_str(&parts[2]).unwrap()),
                _ => unreachable!(),
            }
        } else {
            unreachable!()
        }
    }

    pub fn to_string(&self) -> SharedString {
        match self {
            TreeId::ServiceRoot => SERVICE_ID.into(),
            TreeId::SchemaRoot => SCHEMA_ID.into(),
            TreeId::Service(id) => format!("{}/{}", SERVICE_ID, id).into(),
            TreeId::Schema(id) => format!("{}/{}", SCHEMA_ID, id).into(),
            TreeId::ServiceFolder(id) => format!("{}/folder/{}", SERVICE_ID, id).into(),
            TreeId::SchemaFolder(id) => format!("{}/folder/{}", SCHEMA_ID, id).into(),
        }
    }
}

impl From<SharedString> for TreeId {
    fn from(value: SharedString) -> Self {
        Self::new(value)
    }
}
