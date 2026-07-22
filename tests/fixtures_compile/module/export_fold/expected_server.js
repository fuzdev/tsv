import * as $ from 'svelte/internal/server';
export const a = 'ok';
export default function Input($$renderer) {
	$$renderer.push(`<p>ok</p>`);
}
