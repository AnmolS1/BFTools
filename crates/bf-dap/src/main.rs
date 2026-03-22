fn main() {
    let mut adapter = bf_dap::adapter::DapAdapter::new(bf_dap::transport::DapTransport::stdio());
    adapter.run();
}
