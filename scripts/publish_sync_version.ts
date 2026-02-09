/**
 * Syncs jsr.json version from Cargo.toml workspace version.
 * Source of truth: Cargo.toml [workspace.package] version
 */

const toml = Deno.readTextFileSync('Cargo.toml');
const match = toml.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/);
if (!match) {
	throw new Error('Could not find [workspace.package] version in Cargo.toml');
}
const version = match[1];

const jsr_path = 'crates/tsv_wasm/jsr.json';
const jsr = JSON.parse(Deno.readTextFileSync(jsr_path));

if (jsr.version !== version) {
	jsr.version = version;
	Deno.writeTextFileSync(jsr_path, JSON.stringify(jsr, null, '\t') + '\n');
	console.log(`jsr.json version synced to ${version}`);
} else {
	console.log(`jsr.json version already ${version}`);
}
