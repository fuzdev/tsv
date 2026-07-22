import * as $ from 'svelte/internal/server';
import C from './C.svelte';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	C($$renderer, { x: d() + 1 });
}
