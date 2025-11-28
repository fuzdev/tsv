<style>
	div {
		/* Short transform - stays inline (under 100 chars) */
		transform: translateX(10px) rotate(45deg) scale(1.5);
		/* Transform functions don't wrap arguments - even with long values (130+ chars) */
		transform: matrix(
			0.0000000001,
			1.0000000001,
			2.0000000001,
			3.0000000001,
			4.0000000001,
			5.0000000001
		);
		/* Multiple transform functions in sequence (140+ chars) - tests space-separated wrapping */
		transform: translateX(0px) translateY(0px) rotate(0deg) scale(0) skewX(0deg) skewY(0deg)
			perspective(0px) rotateZ(0deg);
		/* Transform with nested calc expressions (120+ chars) - tests nested function wrapping */
		transform: translate(
			calc(0vw - 0px + 0rem - 0em + 0vmin),
			calc(0vh - 0px + 0rem - 0em + 0vmax + 0ch)
		);
	}
</style>
