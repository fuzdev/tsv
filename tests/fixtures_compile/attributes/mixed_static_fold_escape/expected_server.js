import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = `x"y<z&w`;
	let b = 1;
	$$renderer.push(`<div title="px&quot;y&lt;z&amp;wq1"></div>`);
}
