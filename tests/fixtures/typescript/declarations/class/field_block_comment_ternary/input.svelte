<script lang="ts">
	class A {
		// Ternary with binaryish test: inline comment should use break-after-operator
		prop =
			/* @type {string | ArrayBufferView} */ input instanceof Request && cloned_body ? fn(a) : b;

		// Without comment: same ternary, also breaks after = (binaryish test)
		prop2 =
			input_abcd instanceof Request && cloned_body_abcdefghijklmnopqrstuvwxyzabcdefghijkl
				? fn(a)
				: b;

		// Short case without comment: fits inline
		prop3 = input instanceof Request && cloned_body ? fn(a) : b;
	}
</script>
