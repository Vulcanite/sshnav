import type {Config} from '@docusaurus/types';
import type {Options, ThemeConfig} from '@docusaurus/preset-classic';
import {themes as prismThemes} from 'prism-react-renderer';
import packageJson from './package.json';

const organizationName = process.env.GITHUB_REPOSITORY_OWNER ?? 'Vulcanite';
const projectName = process.env.GITHUB_REPOSITORY?.split('/')[1] ?? 'sshnav';
const repositoryUrl = `https://github.com/${organizationName}/${projectName}`;
const editBranch = process.env.DOCS_EDIT_BRANCH ?? 'main';
const isUserSite = projectName === `${organizationName}.github.io`;

const config: Config = {
  title: 'sshnav',
  tagline: 'Your SSH inventory, one keystroke away.',
  favicon: 'img/favicon.svg',
  url: process.env.DOCS_URL ?? `https://${organizationName}.github.io`,
  baseUrl: process.env.DOCS_BASE_URL ?? (isUserSite ? '/' : `/${projectName}/`),
  organizationName,
  projectName,
  trailingSlash: false,
  onBrokenLinks: 'throw',
  markdown: {
    mermaid: true,
    hooks: {onBrokenMarkdownLinks: 'warn'},
  },
  themes: ['@docusaurus/theme-mermaid'],
  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },
  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          showLastUpdateTime: true,
          editUrl: `${repositoryUrl}/edit/${editBranch}/`,
        },
        blog: false,
        theme: {customCss: './src/css/custom.css'},
        sitemap: {changefreq: 'weekly', priority: 0.5},
      } satisfies Options,
    ],
  ],
  themeConfig: {
    image: 'img/social-card.svg',
    metadata: [
      {name: 'theme-color', content: '#e7704b'},
      {
        name: 'description',
        content: 'Documentation for sshnav, a fast local SSH inventory navigator and launcher.',
      },
    ],
    colorMode: {
      defaultMode: 'dark',
      disableSwitch: false,
      respectPrefersColorScheme: false,
    },
    navbar: {
      title: 'sshnav',
      logo: {alt: 'sshnav prompt mark', src: 'img/logo.svg'},
      items: [
        {to: '/docs/getting-started/quickstart', label: 'Get started', position: 'left'},
        {to: '/docs/guides/jump-hosts', label: 'Guides', position: 'left'},
        {to: '/docs/reference/cli', label: 'CLI', position: 'left'},
        {to: '/docs/changelog', label: 'Changelog', position: 'left'},
        {type: 'html', value: `<span>v${packageJson.version}</span>`, position: 'right'},
        {href: repositoryUrl, label: 'GitHub', position: 'right'},
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Learn',
          items: [
            {label: 'Quickstart', to: '/docs/getting-started/quickstart'},
            {label: 'File transfers', to: '/docs/guides/file-transfers'},
            {label: 'Jump hosts', to: '/docs/guides/jump-hosts'},
          ],
        },
        {
          title: 'Reference',
          items: [
            {label: 'CLI reference', to: '/docs/reference/cli'},
            {label: 'Architecture', to: '/docs/architecture'},
            {label: 'Security', to: '/docs/security'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'GitHub', href: repositoryUrl},
            {label: 'Changelog', to: '/docs/changelog'},
            {label: 'Contributing', to: '/docs/contributing'},
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} sshnav contributors. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['bash', 'toml'],
    },
  } satisfies ThemeConfig,
};

export default config;
