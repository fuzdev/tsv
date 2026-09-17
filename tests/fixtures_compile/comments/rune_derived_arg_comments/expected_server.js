import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let n = 1;
		let double = $.derived((/* twice */) => n * 2);
		let label = $.derived(() =>
			(() => {
				// build the label
				const parts = [n, double()];
				return parts.join(', '); // joined
			})()
		);
		let info = $.derived(() => ({
			// the raw count
			count: n,
			double: double() /* derived */
		}));
		function inc() {
			n += 1;
		}
		$$renderer.push(`<button>${$.escape(label())} ${$.escape(info().count)}</button>`);
	});
}
