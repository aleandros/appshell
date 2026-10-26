use utoipa::OpenApi;
#[expect(
    clippy::print_stdout,
    reason = "This CLI intentionally writes its result to standard output."
)]
fn main() {
    println!(
        "{}",
        appshell_api::ApiDoc::openapi()
            .to_pretty_json()
            .expect("OpenAPI serialization")
    );
}
