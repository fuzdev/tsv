# Unicode Escapes - 7+ Digits (Truncation)

Tests CSS spec truncation: escapes limited to 6 hex digits. Extra digits become literal text.

## Examples

```css
/* 8 digits: \00001F (6) + 4A9 (literal) → U+00001F + "4A9" */
content: '\00001F4A9';

/* 7 digits: \000041 (6) + 9 (literal) → 'A' + "9" */
content: '\0000419';

/* 9 digits: \000000 (6) + ABC (literal) → null + "ABC" */
content: '\000000ABC';
```

**CSS Spec**: Only first 6 hex digits parsed as escape. Remaining become literal characters.
