
#[derive(Debug, Default)]
#[allow(unused)]
pub struct Workspace {
    pub id: usize,
    pub name: String,
    pub visible: bool,
}

#[derive(Debug, Default)]
#[allow(unused)]
pub struct Http {
    pub id: usize,
    pub name: String,
    pub path: String,
    pub method: String,
}

#[derive(Debug, Default)]
#[allow(unused)]
pub struct GRPC {
    pub id: usize,
    pub name: String,
    pub service: String,
    pub method: String,
}

#[derive(Debug, Default)]
#[allow(unused)]
pub struct WebSocket {
    pub id: usize,
    pub name: String,
    pub path: String,
}