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
</script>
