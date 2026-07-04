<svelte:options
	customElement={{
		tag: 'custom-element',
		props: {
			expanded: { reflect: true, type: 'Boolean', attribute: 'aria-expanded' }
		},
		shadow: {
			mode: 'open'
		},
		extend: (customClass) => customClass
	}}
/>
