Feature: Plugin Actions
  As a plugin developer
  I want to test plugin action execution
  So that I can verify my plugin handles requests correctly

  Background:
    Given the plugin "./target/release/libmy_plugin.so" is loaded
    And the plugin is initialized

  @api
  Scenario: Execute action with no arguments
    When I execute the action "info"
    Then the action should succeed
    And the response should be valid JSON
    And the response should have field "name"

  @api
  Scenario: Execute action with JSON arguments
    When I execute the action "process" with arguments:
      """
      {"data": "test", "count": 5}
      """
    Then the action should succeed

  @api
  Scenario: Execute action with inline arguments
    When I execute "echo" with '{"message": "hello"}'
    Then the action should succeed
    And the response should contain "hello"

  @error
  Scenario: Handle invalid action gracefully
    When I execute the action "invalid_action_that_does_not_exist"
    Then the action should fail
    And the error should contain "unknown"

  @error
  Scenario: Handle invalid JSON arguments
    When I execute the action "process" with arguments:
      """
      {invalid json}
      """
    Then the action should fail

  @data
  Scenario: Store and use response data
    When I execute the action "create_item"
    Then the action should succeed
    And the response should have field "id"
    And I store the response field "id" as "item_id"
    When I execute "get_item" with '{"id": "${item_id}"}'
    Then the action should succeed
