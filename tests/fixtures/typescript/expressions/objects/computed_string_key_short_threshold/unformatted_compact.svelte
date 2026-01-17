<script>
// Computed string literal keys: ['x'] = 5 chars (including quotes and brackets)
// Threshold for "short key" is tabWidth (2) + 3 = 5
// So ['x'] (5 chars) is NOT short (5 < 5 = false)

// String literal value with "not short" computed key: breaks after colon
const o1={["x"]:"this is a string literal that should trigger the break path if key is not short xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"};

// Regular identifier key: x = 1 char, IS short
// Same string literal value: no break after colon (but line still breaks because too long)
const o2={x:"this is a string literal that should trigger the break path if key is not short xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"};

// Computed identifier key: [x] = 3 chars, IS short
// Same string literal value: no break after colon
const o3={[x]:"this is a string literal that should trigger the break path if key is not short xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"};

// Longer computed string literal key: ['abcd'] = 8 chars, NOT short
// Line fits at 100 chars, no break needed
const o4={["abcd"]:"this is a string literal that should trigger the break path if key is not short xxx"};
</script>
