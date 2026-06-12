<style>
	div {
		width: calc(100% - 20px);
		width: calc(var(--size) * 2 + 10px);
		width: calc(calc(100px + 50px) * 2);
		color: var(--var1);
		color: var(--var1, blue);
		color: var(--a, var(--b, var(--c, red)));
		color: rgb(var(--r) var(--g) var(--b));
		transform: translate(var(--x), var(--y));
	}
</style>
