import * as $ from 'svelte/internal/server';
export function helper() {}
export default function Input(
	$$renderer // keep me
) {
	let x = 1;
	$$renderer.push(`<p>1</p>`);
}
