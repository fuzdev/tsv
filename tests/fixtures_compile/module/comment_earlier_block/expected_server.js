import * as $ from 'svelte/internal/server';
export function helper() {}
const mid = 1;
// keep me
export const value = mid;
export default function Input($$renderer) {
	$$renderer.push(`<p>hi</p>`);
}
