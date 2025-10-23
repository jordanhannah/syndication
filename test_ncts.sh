#!/bin/bash

# NCTS Connectivity Test Script
# Run this to verify NCTS endpoints are accessible

echo "======================================"
echo "NCTS Connectivity Test"
echo "======================================"
echo ""

NCTS_API_BASE="https://api.healthterminologies.gov.au"
TOKEN_ENDPOINT="https://api.healthterminologies.gov.au/oauth2/token"
SYNDICATION_FEED="https://api.healthterminologies.gov.au/syndication/v1/syndication.xml"

# Load environment variables from .env file
if [ -f .env ]; then
    echo "Loading credentials from .env file..."
    export $(grep -v '^#' .env | grep -v '^$' | xargs)
    echo ""
else
    echo "⚠ Warning: .env file not found"
    echo "  Authentication will not be possible without credentials"
    echo ""
fi

# Check if credentials are set
if [ -z "$NCTS_CLIENT_ID" ] || [ -z "$NCTS_CLIENT_SECRET" ]; then
    echo "⚠ Warning: NCTS credentials not found"
    echo "  Set NCTS_CLIENT_ID and NCTS_CLIENT_SECRET in .env file"
    echo "  Tests will likely fail with 401 errors"
    echo ""
fi

# Function to obtain OAuth2 access token
get_access_token() {
    if [ -z "$NCTS_CLIENT_ID" ] || [ -z "$NCTS_CLIENT_SECRET" ]; then
        return 1
    fi

    echo "Obtaining access token..." >&2

    # Make token request
    response=$(curl -s -w "\n%{http_code}" -X POST "$TOKEN_ENDPOINT" \
        -H "Content-Type: application/x-www-form-urlencoded" \
        -d "grant_type=client_credentials&client_id=$NCTS_CLIENT_ID&client_secret=$NCTS_CLIENT_SECRET" \
        2>&1)

    http_code=$(echo "$response" | tail -n 1)
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" == "200" ]; then
        # Try to extract access_token using grep and sed (works without jq)
        access_token=$(echo "$body" | grep -o '"access_token":"[^"]*"' | head -n 1 | sed 's/"access_token":"\([^"]*\)"/\1/')

        if [ ! -z "$access_token" ]; then
            echo "✓ Access token obtained successfully" >&2
            echo "" >&2
            echo "$access_token"
            return 0
        else
            echo "✗ Failed to parse access token from response" >&2
            echo "  Response body: $body" >&2
            echo "" >&2
            return 1
        fi
    else
        echo "✗ Failed to obtain access token (HTTP $http_code)" >&2
        echo "  Check your NCTS_CLIENT_ID and NCTS_CLIENT_SECRET" >&2
        echo "  Response: $body" >&2
        echo "" >&2
        return 1
    fi
}

# Obtain access token if credentials are available
ACCESS_TOKEN=""
if [ ! -z "$NCTS_CLIENT_ID" ] && [ ! -z "$NCTS_CLIENT_SECRET" ]; then
    ACCESS_TOKEN=$(get_access_token)
    if [ -z "$ACCESS_TOKEN" ]; then
        echo "⚠ Continuing without authentication (tests will likely fail)"
        echo ""
    fi
fi

# Function to test an endpoint
test_endpoint() {
    local name=$1
    local url=$2

    echo "Testing: $name"
    echo "URL: $url"

    # Build curl command with optional Bearer token
    if [ ! -z "$ACCESS_TOKEN" ]; then
        response=$(curl -s -w "\n%{http_code}" -m 10 -H "Authorization: Bearer $ACCESS_TOKEN" "$url" 2>&1)
    else
        response=$(curl -s -w "\n%{http_code}" -m 10 "$url" 2>&1)
    fi
    http_code=$(echo "$response" | tail -n 1)
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" == "200" ]; then
        echo "✓ SUCCESS (HTTP 200)"
        echo "  Feed is accessible"

        # Check if it's valid XML/Atom
        if echo "$body" | grep -q "<feed"; then
            echo "  ✓ Valid Atom feed detected"

            # Try to extract title (BSD grep compatible)
            title=$(echo "$body" | grep -o "<title>[^<]*</title>" | head -1 | sed 's/<title>\(.*\)<\/title>/\1/')
            if [ ! -z "$title" ]; then
                echo "  Feed title: $title"
            fi

            # Count entries
            entries=$(echo "$body" | grep -c "<entry>")
            echo "  Number of entries: $entries"
            echo ""

            # List all entries
            echo "  Listing all entries:"
            echo "  ===================="

            # Save feed to temp file for parsing
            temp_feed=$(mktemp)
            echo "$body" > "$temp_feed"

            # Extract and display each entry
            entry_num=1

            # Use awk to split entries and process them
            awk '/<entry>/{flag=1; entry=""} flag{entry=entry $0 "\n"} /<\/entry>/{flag=0; print "ENTRY_START"; print entry; print "ENTRY_END"}' "$temp_feed" | {
                current_entry=""
                while IFS= read -r line; do
                    if [ "$line" = "ENTRY_START" ]; then
                        current_entry=""
                    elif [ "$line" = "ENTRY_END" ]; then
                        # Process the complete entry
                        if [ ! -z "$current_entry" ]; then
                            # Extract title
                            entry_title=$(echo "$current_entry" | grep -o "<title[^>]*>[^<]*</title>" | sed 's/<title[^>]*>\(.*\)<\/title>/\1/' | head -1)

                            # Extract category term
                            category=$(echo "$current_entry" | grep -o 'term="[^"]*"' | sed 's/term="\([^"]*\)"/\1/' | head -1)

                            # Extract updated date
                            updated=$(echo "$current_entry" | grep -o "<updated>[^<]*</updated>" | sed 's/<updated>\(.*\)<\/updated>/\1/' | head -1)

                            # Extract link href (download URL)
                            link=$(echo "$current_entry" | grep -o 'href="[^"]*"' | sed 's/href="\([^"]*\)"/\1/' | head -1)

                            # Display entry info
                            echo ""
                            echo "  Entry #$entry_num:"
                            if [ ! -z "$entry_title" ]; then
                                echo "    Title:    $entry_title"
                            fi
                            if [ ! -z "$category" ]; then
                                echo "    Category: $category"
                            fi
                            if [ ! -z "$updated" ]; then
                                echo "    Updated:  $updated"
                            fi
                            if [ ! -z "$link" ]; then
                                echo "    URL:      $link"
                            fi

                            entry_num=$((entry_num + 1))
                        fi
                        current_entry=""
                    else
                        current_entry="$current_entry$line"$'\n'
                    fi
                done
            }

            # Clean up temp file
            rm -f "$temp_feed"

            echo ""
            echo "  ===================="
        else
            echo "  ⚠ Response doesn't look like Atom feed"
        fi
    elif [ "$http_code" == "401" ]; then
        echo "✗ AUTHENTICATION REQUIRED (HTTP 401)"
        echo "  You need to provide credentials"
        echo "  See NCTS_INTEGRATION.md for auth setup"
    elif [ "$http_code" == "403" ]; then
        echo "✗ FORBIDDEN (HTTP 403)"
        echo "  Authentication may be working but insufficient permissions"
    elif [ "$http_code" == "404" ]; then
        echo "✗ NOT FOUND (HTTP 404)"
        echo "  This endpoint doesn't exist"
        echo "  Check NCTS documentation for correct URL"
    elif [ "$http_code" == "000" ]; then
        echo "✗ CONNECTION FAILED"
        echo "  Cannot reach server (timeout or network error)"
        echo "  Check your internet connection"
    else
        echo "✗ UNEXPECTED RESPONSE (HTTP $http_code)"
        echo "  Response: $(echo "$body" | head -c 200)"
    fi

    echo ""
}

echo "Testing NCTS Syndication Feed..."
echo "======================================"
echo ""

# Test the unified syndication feed
test_endpoint "Unified Syndication Feed" "$SYNDICATION_FEED"

# Function to show latest SNOMED and FHIR ValueSet entries
show_latest_specific_terminologies() {
    echo ""
    echo "======================================"
    echo "Latest Relevant Versions"
    echo "======================================"
    echo ""

    # Fetch the feed
    if [ ! -z "$ACCESS_TOKEN" ]; then
        response=$(curl -s -w "\n%{http_code}" -m 10 -H "Authorization: Bearer $ACCESS_TOKEN" "$SYNDICATION_FEED" 2>&1)
    else
        echo "⚠ Cannot fetch latest versions without authentication"
        return 1
    fi

    http_code=$(echo "$response" | tail -n 1)
    body=$(echo "$response" | sed '$d')

    if [ "$http_code" != "200" ]; then
        echo "✗ Failed to fetch feed (HTTP $http_code)"
        return 1
    fi

    # Save feed to temp file for parsing
    temp_feed=$(mktemp)
    echo "$body" > "$temp_feed"

    # Arrays to store latest entries (using associative-like pattern)
    latest_snomed_snapshot_date=""
    latest_snomed_snapshot_title=""
    latest_snomed_snapshot_category=""
    latest_snomed_snapshot_updated=""
    latest_snomed_snapshot_url=""

    latest_amt_date=""
    latest_amt_title=""
    latest_amt_category=""
    latest_amt_updated=""
    latest_amt_url=""

    latest_valueset_date=""
    latest_valueset_title=""
    latest_valueset_category=""
    latest_valueset_updated=""
    latest_valueset_url=""

    # Parse entries and find latest SNOMED SNAPSHOT, AMT CSV, and FHIR R4 Bundle
    awk '/<entry>/{flag=1; entry=""} flag{entry=entry $0 "\n"} /<\/entry>/{flag=0; print "ENTRY_START"; print entry; print "ENTRY_END"}' "$temp_feed" | {
        current_entry=""
        while IFS= read -r line; do
            if [ "$line" = "ENTRY_START" ]; then
                current_entry=""
            elif [ "$line" = "ENTRY_END" ]; then
                # Process the complete entry
                if [ ! -z "$current_entry" ]; then
                    # Extract entry details first
                    entry_title=$(echo "$current_entry" | grep -o "<title[^>]*>[^<]*</title>" | sed 's/<title[^>]*>\(.*\)<\/title>/\1/' | head -1)
                    category=$(echo "$current_entry" | grep -o 'term="[^"]*"' | sed 's/term="\([^"]*\)"/\1/' | head -1)

                    # Check if this is a SNOMED SNAPSHOT, AMT CSV, or FHIR R4 Bundle entry
                    is_snomed_snapshot=0
                    is_amt=0
                    is_valueset=0

                    case "$category" in
                        SCT_RF2_SNAPSHOT)
                            # SNAPSHOT format only (DELTA not exposed by server)
                            is_snomed_snapshot=1
                            ;;
                        AMT_CSV)
                            # AMT CSV format only
                            is_amt=1
                            ;;
                        FHIR_Bundle)
                            # Only include R4 NCTS FHIR Bundles, not SNOMED reference set bundles or STU3
                            if echo "$entry_title" | grep -q "(R4)" && ! echo "$entry_title" | grep -q "SNOMED CT-AU Reference Set"; then
                                is_valueset=1
                            fi
                            ;;
                    esac

                    if [ "$is_snomed_snapshot" -eq 1 ] || [ "$is_amt" -eq 1 ] || [ "$is_valueset" -eq 1 ]; then
                        # Extract remaining entry details
                        updated=$(echo "$current_entry" | grep -o "<updated>[^<]*</updated>" | sed 's/<updated>\(.*\)<\/updated>/\1/' | head -1)
                        link=$(echo "$current_entry" | grep -o 'href="[^"]*"' | sed 's/href="\([^"]*\)"/\1/' | head -1)

                        # Convert date to comparable format (ISO 8601 already sortable as string)
                        if [ "$is_snomed_snapshot" -eq 1 ]; then
                            if [ -z "$latest_snomed_snapshot_date" ] || [ "$updated" \> "$latest_snomed_snapshot_date" ]; then
                                latest_snomed_snapshot_date="$updated"
                                latest_snomed_snapshot_title="$entry_title"
                                latest_snomed_snapshot_category="$category"
                                latest_snomed_snapshot_updated="$updated"
                                latest_snomed_snapshot_url="$link"
                            fi
                        fi

                        if [ "$is_amt" -eq 1 ]; then
                            if [ -z "$latest_amt_date" ] || [ "$updated" \> "$latest_amt_date" ]; then
                                latest_amt_date="$updated"
                                latest_amt_title="$entry_title"
                                latest_amt_category="$category"
                                latest_amt_updated="$updated"
                                latest_amt_url="$link"
                            fi
                        fi

                        if [ "$is_valueset" -eq 1 ]; then
                            if [ -z "$latest_valueset_date" ] || [ "$updated" \> "$latest_valueset_date" ]; then
                                latest_valueset_date="$updated"
                                latest_valueset_title="$entry_title"
                                latest_valueset_category="$category"
                                latest_valueset_updated="$updated"
                                latest_valueset_url="$link"
                            fi
                        fi
                    fi
                fi
                current_entry=""
            else
                current_entry="$current_entry$line"$'\n'
            fi
        done

        # Display results
        echo "SNOMED CT-AU (Latest SNAPSHOT):"
        echo "--------------------------------"
        if [ ! -z "$latest_snomed_snapshot_title" ]; then
            echo "  Title:    $latest_snomed_snapshot_title"
            echo "  Category: $latest_snomed_snapshot_category"
            echo "  Updated:  $latest_snomed_snapshot_updated"
            echo "  URL:      $latest_snomed_snapshot_url"
        else
            echo "  No SNOMED CT-AU SNAPSHOT entries found"
        fi
        echo ""

        echo "AMT (Latest CSV):"
        echo "-----------------"
        if [ ! -z "$latest_amt_title" ]; then
            echo "  Title:    $latest_amt_title"
            echo "  Category: $latest_amt_category"
            echo "  Updated:  $latest_amt_updated"
            echo "  URL:      $latest_amt_url"
        else
            echo "  No AMT CSV entries found"
        fi
        echo ""

        echo "NCTS FHIR Bundle R4 (Latest):"
        echo "-----------------------------"
        if [ ! -z "$latest_valueset_title" ]; then
            echo "  Title:    $latest_valueset_title"
            echo "  Category: $latest_valueset_category"
            echo "  Updated:  $latest_valueset_updated"
            echo "  URL:      $latest_valueset_url"
        else
            echo "  No FHIR R4 Bundle entries found"
        fi
        echo ""
    }

    # Clean up temp file
    rm -f "$temp_feed"
}

# Show latest specific terminologies
show_latest_specific_terminologies

echo "======================================"
echo "Test Complete"
echo "======================================"
echo ""
echo "Next Steps:"
echo ""

echo "✓ Syndication feed test complete!"
echo ""
echo "Note: The NCTS uses a single unified feed containing all terminology types."
echo "The app filters entries by category and title to extract:"
echo "  - SNOMED CT-AU (SNAPSHOT format only)"
echo "  - AMT (CSV format only)"
echo "  - FHIR R4 Bundles (excluding SNOMED reference sets)"
echo "  - LOINC is NOT available (proprietary binary format only)"
echo ""
echo "You can now run the app:"
echo "  cargo run"

echo ""
echo "For more help, see:"
echo "  - NCTS_INTEGRATION.md (detailed integration guide)"
echo "  - README.md (general documentation)"
echo "  - QUICKSTART.md (getting started guide)"
