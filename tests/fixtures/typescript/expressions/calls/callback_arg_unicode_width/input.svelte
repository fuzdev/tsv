<script lang="ts">
	// Test unicode width handling at print width boundary (100 chars)
	{
		{
			// ASCII: 100 visual width - stays inline
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevalueval')));

			// ASCII: 101 visual width - breaks
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluevalv')),
			);

			// Emoji (width=2): 100 visual width - stays inline
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluev⭐')));

			// Emoji (width=2): 101 visual width - breaks
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevalueva⭐')),
			);

			// CJK (width=2): 100 visual width - stays inline
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluev中')));

			// CJK (width=2): 101 visual width - breaks
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevalueva中')),
			);

			// Multiple emoji (each 🔥=2): 100 visual width - stays inline
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevv🔥🔥🔥')));

			// Multiple emoji (each 🔥=2): 101 visual width - breaks
			fn(
				a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevalv🔥🔥🔥')),
			);

			// Emoji + skin tone modifier (👋🏽): stays inline (Prettier measures as ~2)
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluev👋🏽')));

			// ZWJ family sequence (👨‍👩‍👧): stays inline (Prettier measures as 2)
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluev👨‍👩‍👧')));

			// Flag emoji (🇺🇸): stays inline (regional indicators 1+1=2)
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluev🇺🇸')));

			// Narrow emoji (❤ width=1): 100 visual width - stays inline
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevalueva❤')));

			// Precomposed é (width=1): 100 visual width - stays inline
			fn(a.method((x) => typeof x === 'string' && x.includes('valuevaluevaluevaluevaluevaluevaé')));
		}
	}
</script>
