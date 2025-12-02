use std::{io::stderr, sync::Arc};

use tlock_pdk::{
    futures::executor::block_on,
    server::ServerBuilder,
    tlock_api::{
        RpcMethod,
        component::{container, text},
        domains::Domain,
        entities::PageId,
        host,
        page::{self, PageEvent},
        plugin,
    },
    wasmi_pdk::{
        rpc_message::RpcError, tracing::info, tracing_subscriber::fmt, transport::JsonRpcTransport,
    },
};

async fn init(transport: Arc<JsonRpcTransport>, _params: ()) -> Result<(), RpcError> {
    host::RegisterEntity
        .call(transport.clone(), Domain::Page)
        .await?;

    Ok(())
}

async fn on_load(transport: Arc<JsonRpcTransport>, page_id: PageId) -> Result<(), RpcError> {
    let component = container(vec![text("Uniswap Plugin")]);
    host::SetPage.call(transport, (page_id, component)).await?;
    Ok(())
}

async fn on_update(
    _transport: Arc<JsonRpcTransport>,
    params: (PageId, PageEvent),
) -> Result<(), RpcError> {
    let (_page_id, _event) = params;
    Ok(())
}

fn main() {
    fmt().with_writer(stderr).init();
    info!("Starting plugin...");

    let reader = std::io::BufReader::new(::std::io::stdin());
    let writer = std::io::stdout();
    let transport = JsonRpcTransport::new(reader, writer);
    let transport = Arc::new(transport);

    let plugin = ServerBuilder::new(transport.clone())
        .with_method(plugin::Init, init)
        .with_method(page::OnLoad, on_load)
        .with_method(page::OnUpdate, on_update)
        .finish();

    let plugin = Arc::new(plugin);

    block_on(async move {
        let _ = transport.process_next_line(Some(plugin)).await;
    });
}
