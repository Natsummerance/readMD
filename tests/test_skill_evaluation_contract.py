import readmd


def test_skill_evaluation_token_is_single_use_and_content_bound():
    token = readmd._issue_skill_evaluation_token('example-skill', 'instructions')
    assert readmd._consume_skill_evaluation_token(token, 'example-skill', 'instructions') is True
    assert readmd._consume_skill_evaluation_token(token, 'example-skill', 'instructions') is False

    changed = readmd._issue_skill_evaluation_token('example-skill', 'instructions')
    assert readmd._consume_skill_evaluation_token(changed, 'example-skill', 'changed') is False


def test_recent_status_rejects_invalid_input_and_bounds_entries():
    api = readmd.Api()
    try:
        api.check_recent_status({'path': 'not-a-list'})
    except ValueError as exc:
        assert str(exc) == 'recent_paths_must_be_list'
    else:
        raise AssertionError('invalid recent input must be rejected')

    paths = [f'C:/ReadMD/{index}.md' for index in range(api.MAX_RECENT_ENTRIES + 10)]
    result = api.check_recent_status(paths)
    assert result['ok'] is True
    assert len(result['items']) == api.MAX_RECENT_ENTRIES
