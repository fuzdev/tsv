import * as $ from 'svelte/internal/server';
export function helper() {}
// keep me
export const value = 1;
export default function Input($$renderer) {
	$$renderer.push(`<p>hi</p>`);
}
