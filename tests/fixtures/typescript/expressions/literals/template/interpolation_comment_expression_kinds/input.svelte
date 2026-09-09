<script lang="ts">
	// A leading block comment the author GLUED to the value occupies the interpolation
	// whatever the value's KIND is: its own newline breaks the `${…}`, and the value follows
	// on the comment's last line. The kinds below are the two halves of prettier's
	// softline-wrap list, which decides the layout only when the interpolation holds no
	// comment at all.

	// Kinds prettier softline-wraps: identifier, member, binary, cast, and a member chain
	// under a non-null (peeled to the member).
	const aaa = `x${
		/* c
d */ bbb
	}y`;
	const ccc = `x${
		/* c
d */ ddd.eee
	}y`;
	const fff = `x${
		/* c
d */ ggg + hhh
	}y`;
	const iii = `x${
		/* c
d */ jjj as Kkk
	}y`;
	const lll = `x${
		/* c
d */ mmm.nnn!
	}y`;

	// Kinds it does not: a non-null, a call, a call under a non-null, an object, an array,
	// and an arrow. The comment breaks the interpolation at each of them too.
	const ooo = `x${
		/* c
d */ ppp!
	}y`;
	const qqq = `x${
		/* c
d */ rrr()
	}y`;
	const sss = `x${
		/* c
d */ ttt(uuu)!
	}y`;
	const vvv = `x${
		/* c
d */ { www: 1 }
	}y`;
	const xxx = `x${
		/* c
d */ [2]
	}y`;
	const yyy = `x${
		/* c
d */ () => zzz
	}y`;
</script>
