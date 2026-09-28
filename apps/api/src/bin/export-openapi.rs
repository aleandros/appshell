use utoipa::OpenApi;
fn main() {
    println!(
        "{}",
        appshell_api::ApiDoc::openapi()
            .to_pretty_json()
            .expect("OpenAPI serialization")
    );
}
