import * as $ from 'svelte/internal/server';
const obj = { el: null };
export default function Input($$renderer) {
	$$renderer.push(`<div>text</div>`);
}
