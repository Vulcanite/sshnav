import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'overview',
    {
      type: 'category',
      label: 'Getting started',
      collapsed: false,
      items: ['getting-started/installation', 'getting-started/quickstart', 'getting-started/core-concepts'],
    },
    {
      type: 'category',
      label: 'Guides',
      items: [
        'guides/managing-hosts',
        'guides/interactive-picker',
        'guides/importing-ssh-config',
        'guides/jump-hosts',
        'guides/file-transfers',
        'guides/private-keys',
        'guides/diagnostics',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: ['reference/cli', 'reference/paths-and-environment'],
    },
    'architecture',
    'security',
    'contributing',
    'changelog',
  ],
};

export default sidebars;
