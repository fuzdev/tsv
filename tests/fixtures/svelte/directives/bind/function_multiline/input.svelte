<!-- Multiline getter/setter bindings: when setter has block body -->

<!-- Block body setter - expands fully -->
<Comp
	bind:value={
		() => a,
		(v) => {
			a = v;
		}
	}
/>

<!-- Multiple statements in setter -->
<Comp
	bind:value={
		() => b,
		(v) => {
			validate(v);
			b = v;
		}
	}
/>
