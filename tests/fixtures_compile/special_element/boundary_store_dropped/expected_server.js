import * as $ from 'svelte/internal/server';
import { w } from './s.js';
export default function Input($$renderer) {
	var $$store_subs;
	$$renderer.push(`<!--[!-->`);
	{
		$$renderer.push(`<i>q</i>`);
	}
	$$renderer.push(`<!--]-->`);
	if ($$store_subs) $.unsubscribe_stores($$store_subs);
}
