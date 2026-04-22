#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]
#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_main)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    mortimmy_rp2350::run(spawner).await
}

#[cfg(not(all(target_arch = "arm", target_os = "none")))]
fn main() {
    mortimmy_rp2350::run_host_stub();
}
