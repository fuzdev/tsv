<script lang="ts">
	// Long member expression - we break when source is compact, prettier keeps inline
	const a = `${
		obj.aaa.bbb.ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
	}`;

	// Multiple interpolations - we break when source is compact, prettier keeps inline
	const b = `${obj.aaa.bbb}__${fn()}__${
		obj.ccc.ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
	}`;

	// Call chain - we break when source is compact, prettier keeps inline
	const c = `${
		obj.fn().method().anotherrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr
	}`;

	// Tagged template - we break when source is compact, prettier keeps inline
	const d = tag`text1 ${
		obj.aaa.bbb.cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
	}`;

	// Logical AND - qualifying, breaks at ${/}
	const e = `${
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	}`;

	// Nullish coalescing - qualifying, breaks at ${/}
	const f = `${
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ?? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	}`;

	// TSAsExpression - qualifying, breaks at ${/}
	const g = `${
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa as BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
	}`;

	// TSSatisfiesExpression - qualifying, breaks at ${/}
	const h = `${
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa satisfies BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
	}`;

	// SequenceExpression - qualifying, breaks at ${/}
	const i = `${
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
	}`;
</script>
