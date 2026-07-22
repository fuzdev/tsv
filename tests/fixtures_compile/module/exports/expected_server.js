import * as $ from 'svelte/internal/server';
export function fn() {}
export let mutable = 1;
export var legacy = 2;
export class Cls {}
const a = 3;
export { a };
export default function Input($$renderer) {
	$$renderer.push(`<p>hi</p>`);
}
