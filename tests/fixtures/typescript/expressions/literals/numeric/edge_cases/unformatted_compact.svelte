<script>
// Zero variations
const zero=0;
const negZero=-0;
const floatZero=0.0;

// Special values (technically identifiers, not literals)
const inf=Infinity;
const negInf=-Infinity;
const notNum=NaN;

// Max safe integer
const maxSafe=9007199254740991;
const minSafe=-9007199254740991;

// Beyond safe integer (precision loss)
const beyondSafe=9007199254740992;

// Very large BigInt with separators
const bigWithSep=1_000_000_000_000_000_000n;

// Hex edge cases
const hexMax32=0xffffffff;
const hexMax64=0xffffffffffffffffn;

// Beyond 2^53 radix literals decode by digit-by-digit double accumulation,
// which can land one ulp below the correctly rounded integer value
const hexBeyondSafe=0x47874750d3a412a2;
const octBeyondSafe=0o75415030007726607105367;

// Binary edge cases
const bin8=0b11111111;
const bin32=0b11111111111111111111111111111111;

// Octal edge cases
const oct8=0o377;

// Scientific notation edge cases
const sciSmall=1e-308;
const sciLarge=1e308;
const sciPrec=1.7976931348623157e308;

// Numeric separators in various bases
const hexSep=0xff_ff_ff_ff;
const binSep=0b1111_1111_1111_1111;
const octSep=0o7_7_7_7;
const sciSep=1_000e3;

// Float edge cases
const smallFloat=0.000001;
const precFloat=0.1+0.2;
</script>
