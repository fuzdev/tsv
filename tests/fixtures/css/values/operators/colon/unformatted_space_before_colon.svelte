<style>
	div {
		/* a `:` inside a function or a group is its own token: it never takes a
		space before and always takes one after */
		margin: f(a :1.5);
		padding: f(a: 1.5);
		width: f(a: 1.5);
		height: (a: 1.5);
		top: f(1.5: 2.5);
		right: f(a: 1.5, b: 2.5);
		/* a url() argument is opaque, so its `:` is content */
		bottom: url(a:1.50);
	}
</style>
