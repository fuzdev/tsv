// U+2028 and U+2029 inside string and template literals count as line
// terminators for line/column tracking, without ending the statement
const a = 'x y';
const b = 'x y';
const c = `x y`;
const d = 1;
