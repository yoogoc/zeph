# Manual Test Plan: HTTP Query and Path Parameters

## Prerequisites

1. Build and run the application:
   ```bash
   cargo run -p zeph
   ```

2. Create a test workspace or use an existing one

3. Have a test API available (or use httpbin.org for testing)

## Test Scenarios

### Scenario 1: Basic Query Parameters

**Objective**: Verify query parameters can be added, edited, and used in requests

**Steps**:
1. Create a new HTTP service with URL: `https://httpbin.org/get`
2. Click on the "Query" tab in the request panel
3. Click "Add Parameter" button
4. Add parameter: `limit` = `10`
5. Add parameter: `page` = `1`
6. Verify URL preview shows: `https://httpbin.org/get?limit=10&page=1`
7. Click "Send" button
8. Verify response shows the query parameters in the `args` field

**Expected Result**: ✅ Query parameters are added to URL and sent correctly

---

### Scenario 2: Query Parameter Enable/Disable

**Objective**: Verify query parameters can be toggled without deletion

**Steps**:
1. Using the service from Scenario 1
2. Click the checkbox next to the `page` parameter to disable it
3. Verify URL preview shows: `https://httpbin.org/get?limit=10` (page removed)
4. Send request and verify only `limit` appears in response
5. Re-enable the `page` parameter
6. Verify URL preview shows both parameters again

**Expected Result**: ✅ Disabled parameters are excluded from URL

---

### Scenario 3: Path Parameters

**Objective**: Verify path parameters are detected and substituted

**Steps**:
1. Create a new HTTP service with URL: `https://httpbin.org/anything/{userId}/posts/{postId}`
2. Verify "Path" tab appears automatically
3. Click on "Path" tab
4. Verify two path parameters are shown: `userId` and `postId`
5. Enter value `123` for `userId`
6. Enter value `456` for `postId`
7. Verify URL preview shows: `https://httpbin.org/anything/123/posts/456`
8. Send request
9. Verify response URL shows substituted values

**Expected Result**: ✅ Path parameters are detected and substituted correctly

---

### Scenario 4: Path Parameter Validation

**Objective**: Verify missing path parameters prevent request execution

**Steps**:
1. Using the service from Scenario 3
2. Clear the value for `postId` (leave it empty)
3. Click "Send" button
4. Verify validation error appears in response panel
5. Verify error message mentions "postId" as missing
6. Fill in `postId` value
7. Send request again
8. Verify request succeeds

**Expected Result**: ✅ Missing path parameters show validation error and block request

---

### Scenario 5: URL Change Detection

**Objective**: Verify path parameters update when URL changes

**Steps**:
1. Create a new HTTP service with URL: `https://httpbin.org/anything/{id}`
2. Verify "Path" tab shows with `id` parameter
3. Change URL to: `https://httpbin.org/anything/{userId}/comments/{commentId}`
4. Click on "Path" tab
5. Verify path parameters now show `userId` and `commentId`
6. Verify old `id` parameter is gone

**Expected Result**: ✅ Path parameters UI updates automatically when URL changes

---

### Scenario 6: Special Character Encoding

**Objective**: Verify special characters are properly encoded

**Steps**:
1. Create HTTP service with URL: `https://httpbin.org/get`
2. Add query parameter: `search` = `hello world` (with space)
3. Add query parameter: `email` = `user@example.com`
4. Add query parameter: `filter` = `name=John&age=30`
5. Verify URL preview shows encoded values
6. Send request
7. Verify response shows correct decoded values in `args`

**Expected Result**: ✅ Special characters are properly URL-encoded

---

### Scenario 7: Folder Parameter Inheritance

**Objective**: Verify folder-level parameters are inherited by services

**Steps**:
1. Create a new folder named "Test API"
2. Edit folder settings (if available) or use existing folder with common parameters
3. Add folder-level common search parameter: `api_key` = `test123`
4. Create a service in this folder with URL: `https://httpbin.org/get`
5. Go to Query tab
6. Verify `api_key` parameter appears with a visual indicator (folder icon or label)
7. Add service-level parameter: `limit` = `10`
8. Verify URL preview shows both parameters
9. Send request and verify both parameters in response

**Expected Result**: ✅ Folder parameters are inherited and displayed with indicator

---

### Scenario 8: Service Parameter Override

**Objective**: Verify service-level parameters override folder parameters

**Steps**:
1. Using the folder and service from Scenario 7
2. Add service-level parameter with same key: `api_key` = `override456`
3. Verify URL preview uses service value: `api_key=override456`
4. Send request and verify service value is used
5. Remove the service-level `api_key` parameter
6. Verify URL preview now shows folder value: `api_key=test123`

**Expected Result**: ✅ Service parameters override folder parameters correctly

---

### Scenario 9: Parameter Persistence

**Objective**: Verify parameters persist across app restarts

**Steps**:
1. Create a service with:
   - URL: `https://httpbin.org/anything/{id}`
   - Path parameter: `id` = `999`
   - Query parameters: `limit=20`, `sort=desc`
2. Note the service name
3. Close the application completely
4. Reopen the application
5. Open the same workspace
6. Open the same service
7. Verify path parameter value is `999`
8. Verify query parameters are `limit=20` and `sort=desc`
9. Verify enabled/disabled states are preserved

**Expected Result**: ✅ All parameters persist correctly across restarts

---

### Scenario 10: OpenAPI Import with Parameters

**Objective**: Verify OpenAPI import extracts parameters correctly

**Steps**:
1. Create an OpenAPI spec file with parameters:
   ```yaml
   openapi: 3.0.0
   info:
     title: Test API
     version: 1.0.0
   paths:
     /users/{userId}:
       get:
         parameters:
           - name: userId
             in: path
             required: true
             schema:
               type: string
           - name: limit
             in: query
             schema:
               type: integer
               default: 10
           - name: offset
             in: query
             schema:
               type: integer
   ```
2. Import this spec into the workspace
3. Open the imported service
4. Verify path parameter `userId` is present
5. Verify query parameters `limit` and `offset` are present
6. Verify default value for `limit` is `10` (if shown)

**Expected Result**: ✅ OpenAPI parameters are extracted and populated correctly

---

### Scenario 11: Combined Path and Query Parameters

**Objective**: Verify path and query parameters work together

**Steps**:
1. Create service with URL: `https://httpbin.org/anything/{category}/{id}`
2. Set path parameters: `category=users`, `id=123`
3. Add query parameters: `include=profile`, `format=json`
4. Verify URL preview shows: `https://httpbin.org/anything/users/123?include=profile&format=json`
5. Send request
6. Verify response URL matches expected URL

**Expected Result**: ✅ Path and query parameters combine correctly

---

### Scenario 12: Unicode and Emoji

**Objective**: Verify Unicode characters are handled correctly

**Steps**:
1. Create service with URL: `https://httpbin.org/get`
2. Add query parameter: `name` = `张三` (Chinese characters)
3. Add query parameter: `emoji` = `🎉🎊`
4. Verify URL preview shows encoded values
5. Send request
6. Verify response shows correct decoded values

**Expected Result**: ✅ Unicode characters are properly encoded and decoded

---

### Scenario 13: Empty and Disabled Parameters

**Objective**: Verify empty and disabled parameters are handled correctly

**Steps**:
1. Create service with URL: `https://httpbin.org/get`
2. Add query parameter with empty key: `` = `value`
3. Add query parameter with empty value: `key` = ``
4. Add query parameter: `test` = `123` and disable it
5. Verify URL preview excludes empty key and disabled parameter
6. Verify URL preview includes parameter with empty value
7. Send request and verify behavior

**Expected Result**: ✅ Empty keys and disabled params excluded, empty values included

---

### Scenario 14: URL Preview Reactivity

**Objective**: Verify URL preview updates immediately on changes

**Steps**:
1. Create service with URL: `https://httpbin.org/anything/{id}`
2. Watch URL preview as you:
   - Type in path parameter value
   - Add query parameter
   - Change query parameter value
   - Toggle query parameter enabled/disabled
   - Change URL to add/remove placeholders
3. Verify preview updates immediately for each change

**Expected Result**: ✅ URL preview is reactive and updates in real-time

---

## Test Results Template

Copy this template to record your test results:

```markdown
## Test Execution Results

**Date**: [Date]
**Tester**: [Name]
**Build**: [Commit hash or version]

### Scenario Results

- [ ] Scenario 1: Basic Query Parameters
- [ ] Scenario 2: Query Parameter Enable/Disable
- [ ] Scenario 3: Path Parameters
- [ ] Scenario 4: Path Parameter Validation
- [ ] Scenario 5: URL Change Detection
- [ ] Scenario 6: Special Character Encoding
- [ ] Scenario 7: Folder Parameter Inheritance
- [ ] Scenario 8: Service Parameter Override
- [ ] Scenario 9: Parameter Persistence
- [ ] Scenario 10: OpenAPI Import with Parameters
- [ ] Scenario 11: Combined Path and Query Parameters
- [ ] Scenario 12: Unicode and Emoji
- [ ] Scenario 13: Empty and Disabled Parameters
- [ ] Scenario 14: URL Preview Reactivity

### Issues Found

1. [Issue description]
   - Severity: [Critical/High/Medium/Low]
   - Steps to reproduce: [Steps]
   - Expected: [Expected behavior]
   - Actual: [Actual behavior]

### Notes

[Any additional observations or comments]
```

## Success Criteria

All scenarios should pass with ✅ results. Any failures should be documented and addressed before marking the feature as complete.
