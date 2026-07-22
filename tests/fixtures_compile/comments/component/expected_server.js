import * as $ from 'svelte/internal/server';
import Child from './Child.svelte';
export default function Input($$renderer) {
	// the label
	let label = 'hi';
	Child($$renderer, { label });
}
