fn main() {
    let cmd: tsv_cli::cli::TopLevel = argh::from_env();
    cmd.run();
}
