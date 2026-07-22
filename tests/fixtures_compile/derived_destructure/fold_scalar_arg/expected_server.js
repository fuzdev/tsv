import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let d = $.derived(() => 5);
	let a = $.derived(() => d().a);
	$$renderer.push(`<!---->5`);
}
