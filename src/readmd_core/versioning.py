# -*- coding: utf-8 -*-
"""Version parsing shared by startup checks and the in-app updater."""

import re

_VERSION_RE = re.compile(
    r'^[vV]?(\d+)(?:\.(\d+))?(?:\.(\d+))?'
    r'(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$'
)


def parse_version(value):
    """Parse a version into comparable core and prerelease tuples."""
    match = _VERSION_RE.match(str(value or '').strip())
    if not match:
        return None

    core = tuple(int(match.group(index) or 0) for index in range(1, 4))
    prerelease = []
    if match.group(4):
        for identifier in match.group(4).split('.'):
            # SemVer gives numeric identifiers lower precedence than words such as beta/rc.
            prerelease.append((0, int(identifier)) if identifier.isdigit() else (1, identifier))
    # The release rank makes an empty prerelease (GA) compare above rc/beta.
    return core, 0 if prerelease else 1, tuple(prerelease)


def compare_versions(left, right):
    """Return -1, 0, or 1; return None when either version cannot be parsed."""
    parsed_left = parse_version(left)
    parsed_right = parse_version(right)
    if parsed_left is None or parsed_right is None:
        return None
    return (parsed_left > parsed_right) - (parsed_left < parsed_right)


def select_update_release(current_version, releases):
    """Select the highest release allowed by the current build's channel.

    Prerelease builds may advance to another prerelease or to a formal release.
    Formal builds stay on the formal channel and therefore never receive a beta
    or release candidate from a release-list response.
    """
    current = parse_version(current_version)
    if current is None:
        return None
    current_is_prerelease = current[1] == 0
    candidates = []
    for release in releases or ():
        if not isinstance(release, dict) or release.get('draft'):
            continue
        parsed = parse_version(release.get('tag_name'))
        if parsed is None:
            continue
        if not current_is_prerelease and (parsed[1] == 0 or release.get('prerelease')):
            continue
        candidates.append((parsed, release))
    return max(candidates, key=lambda item: item[0], default=(None, None))[1]
