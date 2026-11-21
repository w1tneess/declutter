import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';

export default tseslint.config(
  { ignores: ['dist', 'playwright-report', 'test-results', 'src/api/generated'] },
  js.configs.recommended,
  ...tseslint.configs.strict,
  jsxA11y.flatConfigs.strict,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
    plugins: { 'react-hooks': reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // No React Compiler in this build, so its "cannot memoize" notice about useVirtualizer does not apply.
      'react-hooks/incompatible-library': 'off',
      'no-eval': 'error',
      'no-implied-eval': 'error',
      'no-restricted-properties': ['error', { property: 'innerHTML' }, { property: 'outerHTML' }],
      'no-restricted-syntax': [
        'error',
        { selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']", message: 'Never render raw HTML.' },
        { selector: "CallExpression[callee.name='fetch']", message: 'The app makes no network calls.' },
      ],
    },
  },
);
