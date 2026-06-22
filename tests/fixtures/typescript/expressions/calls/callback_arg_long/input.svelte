<script lang="ts">
	// Test callback arg breaking at print width boundary (3 tab indent = 6 visual)
	{
		{
			// 100 chars effective - stays inline
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevalueval')));

			// 101 chars effective - breaks to multiline
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluevalv'))
			);

			// 100 chars by char count but 101 by Unicode width (emoji = 2 cells) - breaks
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevalueva⭐'))
			);

			// 100 chars inner line - breaks outer only
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluevalue'))
			);

			// 101 chars inner line - breaks method call too
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluevaluev'))
			);
		}
	}
</script>
