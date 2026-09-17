import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	// the props
	let {
		label, // shown first
		$$slots,
		$$events,
		// everything else
		...rest // forwarded
	} = $$props;
	function describe() {
		// inner note
		return `${label}`;
	}
	$$renderer.push(`<p${$.attributes({ ...rest })}>${$.escape(describe())}</p>`);
}
