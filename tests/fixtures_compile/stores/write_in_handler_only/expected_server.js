import * as $ from 'svelte/internal/server';
import { count } from './stores.js';
export default function Input($$renderer) {
	var $$store_subs;
	$$renderer.push(`<button>x</button>`);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
