/**
 * Patches wasm-pack's generated package.json for npm publishing.
 *
 * Also copies the variant-specific README into the package root,
 * overwriting any README wasm-pack may have emitted. The crate has no
 * `README.md` at its root — `README_fmt.md` and `README_parse.md` are
 * the canonical sources and ship as the package's `README.md`.
 *
 * For the `parse` variant, also copies `crates/tsv_wasm/types/tsv_ast.d.ts`
 * into the package root alongside the generated `tsv_wasm.d.ts`. The
 * wasm-bindgen `typescript_type = "import('./tsv_ast').*"` extern types
 * resolve against this bundled file at consumer compile time.
 *
 * Usage:  publish_patch_npm.ts <fmt|parse>
 *
 *   fmt   → crates/tsv_wasm/pkg/npm-fmt/    → @fuzdev/tsv_fmt
 *   parse → crates/tsv_wasm/pkg/npm-parse/  → @fuzdev/tsv_parse
 */

const variant = Deno.args[0];
if (variant !== 'fmt' && variant !== 'parse') {
	console.error(`Usage: publish_patch_npm.ts <fmt|parse>`);
	Deno.exit(1);
}

const target_name = variant === 'fmt' ? '@fuzdev/tsv_fmt' : '@fuzdev/tsv_parse';
const pkg_dir = variant === 'fmt' ? 'npm-fmt' : 'npm-parse';
const pkg_root = `crates/tsv_wasm/pkg/${pkg_dir}`;
const pkg_path = `${pkg_root}/package.json`;

const pkg = JSON.parse(Deno.readTextFileSync(pkg_path));
pkg.name = target_name;
pkg.description = variant === 'fmt'
	? 'formatter for TypeScript, Svelte, and CSS'
	: 'parser for TypeScript, Svelte, and CSS';

// Copy variant README into the package root, overwriting any wasm-pack default.
const readme_src = `crates/tsv_wasm/README_${variant}.md`;
const readme_dst = `${pkg_root}/README.md`;
Deno.copyFileSync(readme_src, readme_dst);
console.log(`Copied ${readme_src} → ${readme_dst}`);
if (Array.isArray(pkg.files) && !pkg.files.includes('README.md')) {
	pkg.files.push('README.md');
}

if (variant === 'parse') {
	// Bundle the hand-maintained AST types alongside the generated `tsv_wasm.d.ts`.
	const ast_types_src = `crates/tsv_wasm/types/tsv_ast.d.ts`;
	const ast_types_dst = `${pkg_root}/tsv_ast.d.ts`;
	Deno.copyFileSync(ast_types_src, ast_types_dst);
	console.log(`Copied ${ast_types_src} → ${ast_types_dst}`);

	// wasm-pack's generated package.json lists every file shipped in the
	// tarball. Add `tsv_ast.d.ts` so npm publish picks it up.
	if (Array.isArray(pkg.files) && !pkg.files.includes('tsv_ast.d.ts')) {
		pkg.files.push('tsv_ast.d.ts');
	}
}

Deno.writeTextFileSync(pkg_path, JSON.stringify(pkg, null, '\t') + '\n');
console.log(`Patched ${pkg_path}: name → ${pkg.name}, version ${pkg.version}`);

await print_summary(pkg_root);

async function print_summary(dir: string): Promise<void> {
	const entries = [...Deno.readDirSync(dir)]
		.filter((e) => e.isFile && !e.name.startsWith('.'))
		.map((e) => {
			const path = `${dir}/${e.name}`;
			return { name: e.name, path, size: Deno.statSync(path).size };
		})
		.sort((a, b) => b.size - a.size);

	const wasm = entries.find((e) => e.name.endsWith('.wasm'));
	const wasm_gzipped = wasm ? await gzip_size(wasm.path) : null;

	const name_width = Math.max(...entries.map((e) => e.name.length));
	const size_width = Math.max(...entries.map((e) => format_size(e.size).length));

	console.log(`\nPackage contents (${dir}):`);
	for (const e of entries) {
		const name = e.name.padEnd(name_width);
		const size = format_size(e.size).padStart(size_width);
		const annotation = e === wasm && wasm_gzipped !== null
			? `  →  ${format_size(wasm_gzipped)} gzipped`
			: '';
		console.log(`  ${name}  ${size}${annotation}`);
	}
}

async function gzip_size(path: string): Promise<number> {
	// Shell out to gzip so the number matches `gzip -c | wc -c` (Deno's
	// CompressionStream uses a different default level and reports ~2% high).
	const output = await new Deno.Command('gzip', {
		args: ['-c', path],
		stdout: 'piped',
	}).output();
	return output.stdout.length;
}

function format_size(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}
