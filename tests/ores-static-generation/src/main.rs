use std::{env, fs, path::PathBuf, process};

use ores_api_docs::{
    analyze_generator_source, analyze_page_source, read_page_build_manifest,
    write_page_build_outputs,
};

fn main() {
    valid_dynamic_static_only_route_uses_generator();
    missing_generator_fails_closed();
    reserved_exports_stay_in_their_modules();
    generator_must_be_async();
    revalidation_modes_are_mutually_exclusive();
    println!("fiducia-cloud-test ores static generation certification passed");
}

fn valid_dynamic_static_only_route_uses_generator() {
    let root = fresh_root("valid-static-only");
    write(
        &root,
        "src/pages/articles/[slug]/page.rs",
        r#"#[ores_page(renderer = "mash", delivery = "ssr_only", render = "static_only")]
pub async fn page() {}
"#,
    );
    write(
        &root,
        "src/pages/articles/[slug]/gen.rs",
        r#"#[ores_generate]
pub async fn generate_static_params() {}
"#,
    );

    let outputs = write_page_build_outputs(&root, &root.join(".ores-stack/first-pass"))
        .expect("valid static-only page must plan");
    let manifest = read_page_build_manifest(&outputs.manifest_path).expect("manifest");
    assert_eq!(manifest.routes.len(), 1);
    let route = &manifest.routes[0];
    assert_eq!(route.canonical_path, "/articles/{slug}");
    assert_eq!(route.render, "static_only");
    assert_eq!(route.generator.as_deref(), Some("src/pages/articles/[slug]/gen.rs"));
    assert!(outputs
        .rerun_if_changed
        .iter()
        .any(|path| path.ends_with("src/pages/articles/[slug]/gen.rs")));
    cleanup(root);
}

fn missing_generator_fails_closed() {
    let root = fresh_root("missing-generator");
    write(
        &root,
        "src/pages/articles/[slug]/page.rs",
        r#"#[ores_page(renderer = "mash", delivery = "ssr_only", render = "static_only")]
pub async fn page() {}
"#,
    );
    let error = write_page_build_outputs(&root, &root.join(".ores-stack/first-pass"))
        .expect_err("dynamic static_only route without gen.rs must fail");
    assert!(error.to_string().contains("requires sibling gen.rs"));
    cleanup(root);
}

fn reserved_exports_stay_in_their_modules() {
    let page_error = analyze_page_source(
        "src/pages/page.rs",
        r#"#[ores_page(renderer = "mash", delivery = "ssr_only")]
pub async fn page() {}
#[ores_generate]
pub async fn generate_static_params() {}
"#,
    )
    .expect_err("generate_static_params belongs in gen.rs");
    assert!(page_error.to_string().contains("belongs in sibling gen.rs"));

    let generator_error = analyze_generator_source(
        "src/pages/gen.rs",
        r#"#[ores_generate]
pub async fn generate_static_params() {}
pub async fn page() {}
"#,
    )
    .expect_err("page belongs in page.rs");
    assert!(generator_error.to_string().contains("belongs in page.rs"));
}

fn generator_must_be_async() {
    let error = analyze_generator_source(
        "src/pages/users/[id]/gen.rs",
        r#"#[ores_generate]
pub fn generate_static_params() {}
"#,
    )
    .expect_err("generator must be async");
    assert!(error.to_string().contains("must be async"));
}

fn revalidation_modes_are_mutually_exclusive() {
    let error = analyze_page_source(
        "src/pages/dashboard/page.rs",
        r#"#[ores_page(
renderer = "mash",
delivery = "ssr_only",
revalidate_secs = 60,
on_demand = "dashboard-v1"
)]
pub async fn page() {}
"#,
    )
    .expect_err("time-based and on-demand revalidation cannot both be active");
    assert!(error.to_string().contains("mutually exclusive"));
}

fn write(root: &std::path::Path, relative: &str, content: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, content).expect("write fixture");
}

fn fresh_root(suffix: &str) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "fiducia-ores-static-generation-{}-{now}-{suffix}",
        process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("clear stale fixture");
    }
    root
}

fn cleanup(root: PathBuf) {
    fs::remove_dir_all(root).expect("cleanup fixture");
}
