import * as $ from 'svelte/internal/server';
import { tick } from 'svelte';
export default function Input($$renderer) {
	$$renderer.push(`<p>hi</p>`);
	// after import
}
