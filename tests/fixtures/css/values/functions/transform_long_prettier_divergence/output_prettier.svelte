<style>
	div {
		/* 100 chars - stays inline (at boundary) */
		transform: translateX(0px) translateY(0px) rotate(0deg) scale(0) skewX(0deg) skewY(0deg) aaa(0);

		/* 101 chars - we wrap, Prettier doesn't */
		transform: translateX(0px) translateY(0px) rotate(0deg) scale(0) skewX(0deg) skewY(0deg) aaaa(0);

		/* 102+ chars - wraps with multiple overflow functions */
		transform: translateX(0px) translateY(0px) rotate(0deg) scale(0) skewX(0deg) skewY(0deg) aaaa(0)
			bbbb(0);
	}
</style>
