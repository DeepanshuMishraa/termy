import neonDarkLogo from '../../../assets/legends/neon-logo-dark-color.svg?url';
import neonLightLogo from '../../../assets/legends/neon-logo-light-color.svg?url';

export interface Sponsor {
  name: string;
  /** Shown under the logo; falls back to the name when omitted. */
  description?: string;
  url: string;
  logo: {
    light: string;
    dark: string;
  };
}

export const sponsors: Sponsor[] = [
  {
    name: 'Neon',
    description: 'Serverless Postgres for modern applications.',
    url: 'https://neon.tech',
    logo: {
      light: neonLightLogo,
      dark: neonDarkLogo,
    },
  },
  {
    name: 'Dominik Koch',
    url: 'https://github.com/mezotv',
    logo: {
      light: 'https://github.com/mezotv.png',
      dark: 'https://github.com/mezotv.png',
    },
  },
];
