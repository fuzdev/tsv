<script lang="ts">
	// Long member expression
	const a = `${

		obj.aaa.bbb.ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc

	}`;

	// Multiple interpolations
	const b = `${ obj.aaa.bbb  }__${ fn() }__${

		obj.ccc.ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd

	}`;

	// Call chain
	const c = `${

		obj.fn().method().anotherrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr

	}`;

	// Tagged template
	const d = tag`text1 ${

		obj.aaa.bbb.cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc

	}`;
</script>
